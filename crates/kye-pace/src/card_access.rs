//! EF.CardAccess: `SecurityInfos ::= SET OF SecurityInfo`.

use kye_apdu::tlv::{self, TlvIter};

use crate::PaceError;
use crate::oid::{ID_PACE, Oid, PaceProtocol, StandardizedDomainParameters};

const TAG_SET: u32 = 0x31;
const TAG_SEQUENCE: u32 = 0x30;
const TAG_OID: u32 = 0x06;
const TAG_INTEGER: u32 = 0x02;

/// `PACEInfo ::= SEQUENCE { protocol OID, version INTEGER, parameterId INTEGER OPTIONAL }`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaceInfo {
    pub oid: Oid,
    pub version: u32,
    pub parameter_id: Option<u32>,
}

impl PaceInfo {
    /// Protocol, `None` for PACEDomainParameterInfo-like or unknown OIDs.
    pub fn protocol(&self) -> Option<PaceProtocol> {
        PaceProtocol::from_oid(&self.oid)
    }

    pub fn domain_parameters(&self) -> Option<StandardizedDomainParameters> {
        self.parameter_id.map(StandardizedDomainParameters::from_id)
    }
}

/// One entry of EF.CardAccess.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecurityInfo {
    Pace(PaceInfo),
    /// Any other SecurityInfo (Chip Authentication, domain parameters, ...).
    Other {
        oid: Oid,
    },
}

/// Parsed EF.CardAccess.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardAccess {
    pub infos: Vec<SecurityInfo>,
}

impl CardAccess {
    /// Parses the file content. Trailing bytes after the SET (padding) are ignored.
    pub fn parse(data: &[u8]) -> Result<Self, PaceError> {
        let (set, _) = tlv::parse_one(data)?;
        if set.tag != TAG_SET {
            return Err(PaceError::CardAccess("expected SET OF SecurityInfo"));
        }
        let mut infos = Vec::new();
        for item in set.children() {
            let item = item?;
            if item.tag != TAG_SEQUENCE {
                return Err(PaceError::CardAccess("expected SEQUENCE"));
            }
            infos.push(parse_security_info(item.value)?);
        }
        Ok(Self { infos })
    }

    pub fn pace_infos(&self) -> impl Iterator<Item = &PaceInfo> {
        self.infos.iter().filter_map(|i| match i {
            SecurityInfo::Pace(p) => Some(p),
            SecurityInfo::Other { .. } => None,
        })
    }

    /// First PACEInfo announcing `protocol`.
    pub fn find_pace(&self, protocol: PaceProtocol) -> Option<&PaceInfo> {
        self.pace_infos().find(|p| p.protocol() == Some(protocol))
    }

    /// Total encoded length announced by the outer TLV header, if the header
    /// is complete. Used to read the file without reading to EOF.
    pub fn encoded_len(prefix: &[u8]) -> Option<usize> {
        let (&tag, rest) = prefix.split_first()?;
        let (&first, rest) = rest.split_first()?;
        if tag != TAG_SET as u8 {
            return None;
        }
        if first < 0x80 {
            return Some(2 + usize::from(first));
        }
        let n = usize::from(first & 0x7F);
        if n == 0 || n > 3 || rest.len() < n {
            return None;
        }
        let len = rest[..n]
            .iter()
            .fold(0usize, |acc, b| (acc << 8) | usize::from(*b));
        Some(2 + n + len)
    }
}

fn parse_security_info(seq: &[u8]) -> Result<SecurityInfo, PaceError> {
    let mut it = TlvIter::new(seq);
    let oid = match it.next() {
        Some(Ok(t)) if t.tag == TAG_OID => Oid(t.value.to_vec()),
        Some(Err(e)) => return Err(e.into()),
        _ => return Err(PaceError::CardAccess("SecurityInfo without OID")),
    };
    // PACEInfo OIDs are id-PACE.<mapping>.<cipher>; id-PACE.<mapping> alone
    // denotes PACEDomainParameterInfo.
    if !oid.starts_with(&ID_PACE) || oid.as_bytes().len() != ID_PACE.len() + 2 {
        return Ok(SecurityInfo::Other { oid });
    }
    let version = match it.next() {
        Some(Ok(t)) if t.tag == TAG_INTEGER => parse_uint(t.value)?,
        Some(Err(e)) => return Err(e.into()),
        _ => return Err(PaceError::CardAccess("PACEInfo without version")),
    };
    let parameter_id = match it.next() {
        Some(Ok(t)) if t.tag == TAG_INTEGER => Some(parse_uint(t.value)?),
        Some(Err(e)) => return Err(e.into()),
        _ => None,
    };
    Ok(SecurityInfo::Pace(PaceInfo {
        oid,
        version,
        parameter_id,
    }))
}

fn parse_uint(v: &[u8]) -> Result<u32, PaceError> {
    if v.is_empty() || v.len() > 5 || v[0] & 0x80 != 0 {
        return Err(PaceError::CardAccess("unsupported INTEGER"));
    }
    let n = v.iter().fold(0u64, |acc, b| (acc << 8) | u64::from(*b));
    u32::try_from(n).map_err(|_| PaceError::CardAccess("INTEGER too large"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic EF.CardAccess with one ECDH-GM AES-128 PACEInfo (brainpoolP256r1)
    /// and one unrelated SecurityInfo.
    const CARD_ACCESS: &[u8] = &[
        0x31, 0x22, //
        0x30, 0x12, //
        0x06, 0x0A, 0x04, 0x00, 0x7F, 0x00, 0x07, 0x02, 0x02, 0x04, 0x02, 0x02, //
        0x02, 0x01, 0x02, //
        0x02, 0x01, 0x0D, //
        0x30, 0x0C, //
        0x06, 0x0A, 0x04, 0x00, 0x7F, 0x00, 0x07, 0x02, 0x02, 0x03, 0x02, 0x02, //
    ];

    #[test]
    fn parse_pace_info() {
        let ca = CardAccess::parse(CARD_ACCESS).unwrap();
        assert_eq!(ca.infos.len(), 2);
        let pace = ca.find_pace(PaceProtocol::ECDH_GM_AES_128).unwrap();
        assert_eq!(pace.version, 2);
        assert_eq!(pace.parameter_id, Some(13));
        assert_eq!(
            pace.domain_parameters(),
            Some(StandardizedDomainParameters::BrainpoolP256r1)
        );
        assert!(matches!(ca.infos[1], SecurityInfo::Other { .. }));
    }

    #[test]
    fn encoded_len() {
        assert_eq!(
            CardAccess::encoded_len(CARD_ACCESS),
            Some(CARD_ACCESS.len())
        );
        assert_eq!(
            CardAccess::encoded_len(&[0x31, 0x82, 0x01, 0x00]),
            Some(260)
        );
        assert_eq!(CardAccess::encoded_len(&[0x30, 0x01]), None);
        assert_eq!(CardAccess::encoded_len(&[0x31]), None);
    }

    #[test]
    fn trailing_padding_ignored() {
        let mut data = CARD_ACCESS.to_vec();
        data.extend_from_slice(&[0x00, 0x00]);
        assert!(CardAccess::parse(&data).is_ok());
    }

    #[test]
    fn rejects_garbage() {
        assert!(CardAccess::parse(&[0x30, 0x00]).is_err());
        assert!(CardAccess::parse(&[0x31, 0x02, 0x02, 0x00]).is_err());
    }
}

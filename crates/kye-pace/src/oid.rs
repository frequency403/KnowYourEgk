//! Object identifiers used by PACE (BSI TR-03110 / ICAO 9303 Part 11).

use std::fmt;

/// DER content bytes of `id-PACE` = 0.4.0.127.0.7.2.2.4.
pub const ID_PACE: [u8; 8] = [0x04, 0x00, 0x7F, 0x00, 0x07, 0x02, 0x02, 0x04];

/// DER content bytes of an object identifier (without tag and length).
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Oid(pub Vec<u8>);

impl Oid {
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn starts_with(&self, prefix: &[u8]) -> bool {
        self.0.starts_with(prefix)
    }

    /// Decodes the arcs, `None` if the encoding is malformed.
    pub fn arcs(&self) -> Option<Vec<u64>> {
        let (&first, rest) = self.0.split_first()?;
        let mut arcs = vec![u64::from(first / 40).min(2)];
        arcs.push(u64::from(first) - arcs[0] * 40);
        let mut acc: u64 = 0;
        let mut pending = false;
        for &b in rest {
            acc = acc.checked_mul(128)? | u64::from(b & 0x7F);
            pending = b & 0x80 != 0;
            if !pending {
                arcs.push(acc);
                acc = 0;
            }
        }
        (!pending).then_some(arcs)
    }
}

impl fmt::Display for Oid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.arcs() {
            Some(arcs) => {
                let s: Vec<String> = arcs.iter().map(u64::to_string).collect();
                f.write_str(&s.join("."))
            }
            None => write!(f, "invalid OID {}", kye_apdu::Hex(&self.0)),
        }
    }
}

impl fmt::Debug for Oid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Oid({self})")
    }
}

/// Key agreement / mapping of a PACE protocol OID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mapping {
    DhGm,
    EcdhGm,
    DhIm,
    EcdhIm,
    EcdhCam,
}

/// Cipher and MAC of a PACE protocol OID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cipher {
    TripleDesCbcCbc,
    AesCbcCmac128,
    AesCbcCmac192,
    AesCbcCmac256,
}

/// A decoded `id-PACE-*` protocol identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaceProtocol {
    pub mapping: Mapping,
    pub cipher: Cipher,
}

impl PaceProtocol {
    /// `id-PACE-ECDH-GM-AES-CBC-CMAC-128`, the protocol of the eGK G2.1.
    pub const ECDH_GM_AES_128: Self = Self {
        mapping: Mapping::EcdhGm,
        cipher: Cipher::AesCbcCmac128,
    };

    pub fn from_oid(oid: &Oid) -> Option<Self> {
        let rest = oid.as_bytes().strip_prefix(ID_PACE.as_slice())?;
        let [m, c] = rest else { return None };
        let mapping = match m {
            1 => Mapping::DhGm,
            2 => Mapping::EcdhGm,
            3 => Mapping::DhIm,
            4 => Mapping::EcdhIm,
            6 => Mapping::EcdhCam,
            _ => return None,
        };
        let cipher = match c {
            1 => Cipher::TripleDesCbcCbc,
            2 => Cipher::AesCbcCmac128,
            3 => Cipher::AesCbcCmac192,
            4 => Cipher::AesCbcCmac256,
            _ => return None,
        };
        Some(Self { mapping, cipher })
    }
}

/// Standardized domain parameter IDs (BSI TR-03110 Part 3, Table 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandardizedDomainParameters {
    BrainpoolP256r1,
    BrainpoolP384r1,
    BrainpoolP512r1,
    NistP256,
    NistP384,
    NistP521,
    Other(u32),
}

impl StandardizedDomainParameters {
    pub fn from_id(id: u32) -> Self {
        match id {
            13 => Self::BrainpoolP256r1,
            16 => Self::BrainpoolP384r1,
            17 => Self::BrainpoolP512r1,
            12 => Self::NistP256,
            15 => Self::NistP384,
            18 => Self::NistP521,
            other => Self::Other(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn egk_protocol_oid() {
        let oid = Oid(vec![
            0x04, 0x00, 0x7F, 0x00, 0x07, 0x02, 0x02, 0x04, 0x02, 0x02,
        ]);
        assert_eq!(oid.to_string(), "0.4.0.127.0.7.2.2.4.2.2");
        assert_eq!(
            PaceProtocol::from_oid(&oid),
            Some(PaceProtocol::ECDH_GM_AES_128)
        );
    }

    #[test]
    fn multibyte_arc() {
        // 1.2.840.113549
        let oid = Oid(vec![0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D]);
        assert_eq!(oid.to_string(), "1.2.840.113549");
        assert!(Oid(vec![0x2A, 0x86]).arcs().is_none());
    }

    #[test]
    fn non_pace_oid() {
        let oid = Oid(vec![
            0x04, 0x00, 0x7F, 0x00, 0x07, 0x02, 0x02, 0x03, 0x02, 0x02,
        ]);
        assert_eq!(PaceProtocol::from_oid(&oid), None);
    }
}

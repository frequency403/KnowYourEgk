//! EF.StatusVD (25 bytes):
//! `status(1) || timestamp(14) || versionXml(5, BCD) || versionStorage(5, BCD)`.

use std::fmt;

use crate::EgkError;

pub const STATUS_VD_LEN: usize = 25;

const FILE: &str = "EF.StatusVD";

/// A 10-digit BCD version number.
///
/// Rendered as `XXX.YYY.ZZZZ` (major, minor, revision), leading zeros
/// stripped per field. The 3/3/4 split is an assumption that still needs to
/// be checked against gemSpec_eGK_Fach_VSDM and a real card.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct BcdVersion(pub [u8; 5]);

impl BcdVersion {
    /// The ten decimal digits, `None` if any nibble is not a decimal digit.
    pub fn digits(&self) -> Option<[u8; 10]> {
        let mut d = [0u8; 10];
        for (i, b) in self.0.iter().enumerate() {
            let (hi, lo) = (b >> 4, b & 0x0F);
            if hi > 9 || lo > 9 {
                return None;
            }
            d[2 * i] = hi;
            d[2 * i + 1] = lo;
        }
        Some(d)
    }

    /// `(major, minor, revision)`.
    pub fn parts(&self) -> Option<(u32, u32, u32)> {
        let d = self.digits()?;
        let num = |s: &[u8]| s.iter().fold(0u32, |acc, x| acc * 10 + u32::from(*x));
        Some((num(&d[0..3]), num(&d[3..6]), num(&d[6..10])))
    }
}

impl fmt::Display for BcdVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.parts() {
            Some((a, b, c)) => write!(f, "{a}.{b}.{c}"),
            None => write!(f, "invalid BCD {}", kye_apdu::Hex(&self.0)),
        }
    }
}

impl fmt::Debug for BcdVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BcdVersion({self})")
    }
}

/// UTC timestamp `YYYYMMDDhhmmss` of the last VSD update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timestamp {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }
}

/// Parsed EF.StatusVD.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusVd {
    /// `true` if the last VSD update did not complete (status `'1'`).
    pub update_incomplete: bool,
    pub timestamp: Timestamp,
    pub version_xml: BcdVersion,
    pub version_storage: BcdVersion,
}

impl StatusVd {
    pub fn parse(data: &[u8]) -> Result<Self, EgkError> {
        let data: &[u8; STATUS_VD_LEN] = data
            .get(..STATUS_VD_LEN)
            .and_then(|d| d.try_into().ok())
            .ok_or(EgkError::Format {
                file: FILE,
                reason: "shorter than 25 bytes",
            })?;
        let update_incomplete = match data[0] {
            b'0' => false,
            b'1' => true,
            _ => {
                return Err(EgkError::Format {
                    file: FILE,
                    reason: "status is neither '0' nor '1'",
                });
            }
        };
        let timestamp = parse_timestamp(&data[1..15])?;
        let mut xml = [0u8; 5];
        xml.copy_from_slice(&data[15..20]);
        let mut storage = [0u8; 5];
        storage.copy_from_slice(&data[20..25]);
        Ok(Self {
            update_incomplete,
            timestamp,
            version_xml: BcdVersion(xml),
            version_storage: BcdVersion(storage),
        })
    }
}

fn parse_timestamp(ascii: &[u8]) -> Result<Timestamp, EgkError> {
    let bad = EgkError::Format {
        file: FILE,
        reason: "timestamp is not YYYYMMDDhhmmss",
    };
    if ascii.len() != 14 || !ascii.iter().all(u8::is_ascii_digit) {
        return Err(bad);
    }
    let n = |r: std::ops::Range<usize>| {
        ascii[r]
            .iter()
            .fold(0u16, |acc, d| acc * 10 + u16::from(d - b'0'))
    };
    let ts = Timestamp {
        year: n(0..4),
        month: n(4..6) as u8,
        day: n(6..8) as u8,
        hour: n(8..10) as u8,
        minute: n(10..12) as u8,
        second: n(12..14) as u8,
    };
    let valid = (1..=12).contains(&ts.month)
        && (1..=31).contains(&ts.day)
        && ts.hour < 24
        && ts.minute < 60
        && ts.second < 61;
    if valid { Ok(ts) } else { Err(bad) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(status: u8) -> Vec<u8> {
        let mut v = vec![status];
        v.extend_from_slice(b"20240131235959");
        v.extend_from_slice(&[0x00, 0x50, 0x02, 0x00, 0x00]);
        v.extend_from_slice(&[0x00, 0x10, 0x00, 0x00, 0x01]);
        v
    }

    #[test]
    fn parse_ok() {
        let s = StatusVd::parse(&fixture(b'0')).unwrap();
        assert!(!s.update_incomplete);
        assert_eq!(s.timestamp.to_string(), "2024-01-31 23:59:59 UTC");
        assert_eq!(s.version_xml.to_string(), "5.2.0");
        assert_eq!(s.version_storage.to_string(), "1.0.1");
        assert!(StatusVd::parse(&fixture(b'1')).unwrap().update_incomplete);
    }

    #[test]
    fn parse_errors() {
        assert!(StatusVd::parse(&fixture(b'2')).is_err());
        assert!(StatusVd::parse(&fixture(b'0')[..24]).is_err());
        let mut f = fixture(b'0');
        f[5] = b'1';
        f[6] = b'3';
        assert!(StatusVd::parse(&f).is_err());
        assert_eq!(BcdVersion([0xFA, 0, 0, 0, 0]).parts(), None);
    }
}

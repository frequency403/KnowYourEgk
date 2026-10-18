use std::fmt;

use crate::ApduError;

/// Status word SW1 SW2.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct StatusWord(pub u16);

impl StatusWord {
    pub const OK: Self = Self(0x9000);
    /// End of file reached before Ne bytes were read.
    pub const END_OF_FILE: Self = Self(0x6282);
    pub const WRONG_LENGTH: Self = Self(0x6700);
    pub const SECURITY_STATUS_NOT_SATISFIED: Self = Self(0x6982);
    pub const FILE_NOT_FOUND: Self = Self(0x6A82);
    pub const INS_NOT_SUPPORTED: Self = Self(0x6D00);

    pub fn from_bytes(sw1: u8, sw2: u8) -> Self {
        Self(u16::from_be_bytes([sw1, sw2]))
    }

    pub fn sw1(self) -> u8 {
        (self.0 >> 8) as u8
    }

    pub fn sw2(self) -> u8 {
        self.0 as u8
    }

    pub fn is_ok(self) -> bool {
        self == Self::OK
    }

    /// `61 XX`: more data available via GET RESPONSE.
    pub fn bytes_remaining(self) -> Option<u8> {
        (self.sw1() == 0x61).then_some(self.sw2())
    }

    /// `63 CX`: verification failed, X retries left.
    pub fn retry_counter(self) -> Option<u8> {
        (self.sw1() == 0x63 && self.sw2() & 0xF0 == 0xC0).then_some(self.sw2() & 0x0F)
    }
}

impl fmt::Display for StatusWord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02X} {:02X}", self.sw1(), self.sw2())
    }
}

impl fmt::Debug for StatusWord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StatusWord({self})")
    }
}

/// A response APDU: data field plus status word.
#[derive(Clone, PartialEq, Eq)]
pub struct Response {
    pub data: Vec<u8>,
    pub sw: StatusWord,
}

impl Response {
    pub fn new(data: impl Into<Vec<u8>>, sw: StatusWord) -> Self {
        Self {
            data: data.into(),
            sw,
        }
    }

    pub fn from_bytes(raw: &[u8]) -> Result<Self, ApduError> {
        let split = raw
            .len()
            .checked_sub(2)
            .ok_or(ApduError::ResponseTooShort)?;
        let (data, sw) = raw.split_at(split);
        Ok(Self {
            data: data.to_vec(),
            sw: StatusWord::from_bytes(sw[0], sw[1]),
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = self.data.clone();
        out.extend_from_slice(&self.sw.0.to_be_bytes());
        out
    }
}

// Response data may contain personal data, so Debug only shows its length.
impl fmt::Debug for Response {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Response")
            .field("data_len", &self.data.len())
            .field("sw", &self.sw)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        let r = Response::from_bytes(&[0x01, 0x02, 0x90, 0x00]).unwrap();
        assert_eq!(r.data, [1, 2]);
        assert!(r.sw.is_ok());
        assert_eq!(r.to_bytes(), [0x01, 0x02, 0x90, 0x00]);
        assert_eq!(
            Response::from_bytes(&[0x90]),
            Err(ApduError::ResponseTooShort)
        );
    }

    #[test]
    fn sw_helpers() {
        assert_eq!(StatusWord(0x63C2).retry_counter(), Some(2));
        assert_eq!(StatusWord(0x6310).retry_counter(), None);
        assert_eq!(StatusWord(0x6110).bytes_remaining(), Some(0x10));
        assert_eq!(StatusWord(0x6982).to_string(), "69 82");
    }
}

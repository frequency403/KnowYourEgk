use crate::ApduError;

/// Largest Ne expressible in a short APDU (Le = `00`).
pub const MAX_NE_SHORT: usize = 256;
/// Largest Ne expressible in an extended APDU (Le = `00 00`).
pub const MAX_NE_EXTENDED: usize = 65536;

const MAX_NC_EXTENDED: usize = 65535;

/// Length-field encoding of a command APDU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    /// Short if possible, extended otherwise.
    Auto,
    Short,
    Extended,
}

/// A command APDU.
///
/// `ne` is the expected response length Ne (not the encoded Le byte):
/// `Some(256)` encodes as short Le `00`, `Some(65536)` as extended Le `00 00`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub cla: u8,
    pub ins: u8,
    pub p1: u8,
    pub p2: u8,
    pub data: Vec<u8>,
    pub ne: Option<usize>,
    pub encoding: Encoding,
}

impl Command {
    /// Case 1 command (header only).
    pub fn new(cla: u8, ins: u8, p1: u8, p2: u8) -> Self {
        Self {
            cla,
            ins,
            p1,
            p2,
            data: Vec::new(),
            ne: None,
            encoding: Encoding::Auto,
        }
    }

    pub fn with_data(mut self, data: impl Into<Vec<u8>>) -> Self {
        self.data = data.into();
        self
    }

    pub fn with_ne(mut self, ne: usize) -> Self {
        self.ne = Some(ne);
        self
    }

    pub fn with_encoding(mut self, encoding: Encoding) -> Self {
        self.encoding = encoding;
        self
    }

    /// The four header bytes CLA INS P1 P2.
    pub fn header(&self) -> [u8; 4] {
        [self.cla, self.ins, self.p1, self.p2]
    }

    /// ISO 7816-4 case number (1..=4).
    pub fn case(&self) -> u8 {
        match (self.data.is_empty(), self.ne.is_some()) {
            (true, false) => 1,
            (true, true) => 2,
            (false, false) => 3,
            (false, true) => 4,
        }
    }

    /// Whether this command will be serialised with extended length fields.
    pub fn is_extended(&self) -> Result<bool, ApduError> {
        let nc = self.data.len();
        if nc > MAX_NC_EXTENDED {
            return Err(ApduError::DataTooLong(nc));
        }
        if let Some(ne) = self.ne
            && (ne == 0 || ne > MAX_NE_EXTENDED)
        {
            return Err(ApduError::NeTooLarge(ne));
        }
        let fits_short = nc <= 255 && self.ne.is_none_or(|ne| ne <= MAX_NE_SHORT);
        match self.encoding {
            Encoding::Auto => Ok(!fits_short),
            Encoding::Extended => Ok(self.case() != 1),
            Encoding::Short if fits_short => Ok(false),
            Encoding::Short => Err(ApduError::NotShortEncodable { nc, ne: self.ne }),
        }
    }

    /// Serialises the command.
    pub fn to_bytes(&self) -> Result<Vec<u8>, ApduError> {
        let extended = self.is_extended()?;
        let nc = self.data.len();
        let mut out = Vec::with_capacity(4 + 3 + nc + 3);
        out.extend_from_slice(&self.header());

        if nc > 0 {
            if extended {
                out.push(0x00);
                out.extend_from_slice(&(nc as u16).to_be_bytes());
            } else {
                out.push(nc as u8);
            }
            out.extend_from_slice(&self.data);
        }

        if let Some(ne) = self.ne {
            if extended {
                // In case 2E the Le field carries a leading 00; in case 4E it does not.
                if nc == 0 {
                    out.push(0x00);
                }
                // 65536 is encoded as 00 00 by truncation.
                out.extend_from_slice(&((ne & 0xFFFF) as u16).to_be_bytes());
            } else {
                out.push((ne & 0xFF) as u8);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case1() {
        assert_eq!(
            Command::new(0x00, 0xA4, 0x00, 0x0C).to_bytes().unwrap(),
            [0x00, 0xA4, 0x00, 0x0C]
        );
    }

    #[test]
    fn case2_short_and_extended() {
        let c = Command::new(0x00, 0xB0, 0x8C, 0x00).with_ne(25);
        assert_eq!(c.to_bytes().unwrap(), [0x00, 0xB0, 0x8C, 0x00, 0x19]);
        let c = Command::new(0x00, 0xB0, 0x8C, 0x00).with_ne(256);
        assert_eq!(c.to_bytes().unwrap(), [0x00, 0xB0, 0x8C, 0x00, 0x00]);
        let c = Command::new(0x00, 0xB0, 0x9C, 0x00).with_ne(MAX_NE_EXTENDED);
        assert_eq!(
            c.to_bytes().unwrap(),
            [0x00, 0xB0, 0x9C, 0x00, 0x00, 0x00, 0x00]
        );
        let c = Command::new(0x00, 0xB0, 0x9C, 0x00)
            .with_ne(25)
            .with_encoding(Encoding::Extended);
        assert_eq!(
            c.to_bytes().unwrap(),
            [0x00, 0xB0, 0x9C, 0x00, 0x00, 0x00, 0x19]
        );
    }

    #[test]
    fn case3_and_case4() {
        let aid = [0xD2, 0x76, 0x00, 0x00, 0x01, 0x02];
        let c = Command::new(0x00, 0xA4, 0x04, 0x0C).with_data(aid);
        assert_eq!(
            c.to_bytes().unwrap(),
            [
                0x00, 0xA4, 0x04, 0x0C, 0x06, 0xD2, 0x76, 0x00, 0x00, 0x01, 0x02
            ]
        );
        let c = Command::new(0x10, 0x86, 0x00, 0x00)
            .with_data([0x7C, 0x00])
            .with_ne(256);
        assert_eq!(
            c.to_bytes().unwrap(),
            [0x10, 0x86, 0x00, 0x00, 0x02, 0x7C, 0x00, 0x00]
        );
        let c = Command::new(0x0C, 0xB0, 0x00, 0x00)
            .with_data([0x97, 0x01, 0x00])
            .with_ne(MAX_NE_EXTENDED);
        assert_eq!(
            c.to_bytes().unwrap(),
            [
                0x0C, 0xB0, 0x00, 0x00, 0x00, 0x00, 0x03, 0x97, 0x01, 0x00, 0x00, 0x00
            ]
        );
    }

    #[test]
    fn short_rejects_large_ne() {
        let c = Command::new(0x00, 0xB0, 0x00, 0x00)
            .with_ne(300)
            .with_encoding(Encoding::Short);
        assert!(matches!(
            c.to_bytes(),
            Err(ApduError::NotShortEncodable { .. })
        ));
        assert!(Command::new(0, 0, 0, 0).with_ne(0).to_bytes().is_err());
    }

    #[test]
    fn cases() {
        assert_eq!(Command::new(0, 0, 0, 0).case(), 1);
        assert_eq!(Command::new(0, 0, 0, 0).with_ne(1).case(), 2);
        assert_eq!(Command::new(0, 0, 0, 0).with_data([1]).case(), 3);
        assert_eq!(Command::new(0, 0, 0, 0).with_data([1]).with_ne(1).case(), 4);
    }
}

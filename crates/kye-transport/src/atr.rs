//! Interface detection from the ATR.
//!
//! Detection deliberately does not look at reader names, which differ per OS.

/// Physical interface a card is reached through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardInterface {
    Contact,
    Contactless,
    Unknown,
}

/// Historical-byte pattern of the eGK contact ATR.
const EGK_CONTACT_MARKER: [u8; 5] = [0x81, 0xB1, 0xFE, 0x45, 0x1F];

impl CardInterface {
    pub fn from_atr(atr: &[u8]) -> Self {
        match atr {
            // PC/SC Part 3 pseudo-ATR for contactless cards: 3B 8n 80 01 ...
            [0x3B, t0, 0x80, 0x01, ..] if t0 & 0xF0 == 0x80 => Self::Contactless,
            [0x3B | 0x3F, ..] => Self::Contact,
            _ => Self::Unknown,
        }
    }
}

/// Whether `atr` looks like the contact ATR of an eGK.
pub fn is_egk_contact_atr(atr: &[u8]) -> bool {
    CardInterface::from_atr(atr) == CardInterface::Contact
        && atr
            .windows(EGK_CONTACT_MARKER.len())
            .any(|w| w == EGK_CONTACT_MARKER)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contactless_pseudo_atr() {
        // Synthetic ISO 14443-4 type B pseudo-ATR.
        let atr = [
            0x3B, 0x88, 0x80, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x09,
        ];
        assert_eq!(CardInterface::from_atr(&atr), CardInterface::Contactless);
    }

    #[test]
    fn contact_atr() {
        let atr = [
            0x3B, 0xD3, 0x96, 0xFF, 0x81, 0xB1, 0xFE, 0x45, 0x1F, 0x07, 0x80,
        ];
        assert_eq!(CardInterface::from_atr(&atr), CardInterface::Contact);
        assert!(is_egk_contact_atr(&atr));
        assert!(!is_egk_contact_atr(&[0x3B, 0x90, 0x00]));
    }

    #[test]
    fn unknown() {
        assert_eq!(CardInterface::from_atr(&[]), CardInterface::Unknown);
        assert_eq!(
            CardInterface::from_atr(&[0x00, 0x11]),
            CardInterface::Unknown
        );
    }
}

//! Plain command APDUs used to navigate and read the eGK.

use kye_apdu::Command;

use crate::objects::AID_HCA;

/// Largest offset addressable with P1/P2 READ BINARY (15 bit).
pub const MAX_OFFSET: u16 = 0x7FFF;

/// `SELECT` the MF (no data, no response data).
pub fn select_mf() -> Command {
    Command::new(0x00, 0xA4, 0x04, 0x0C)
}

/// `SELECT DF.HCA` by AID without response data.
pub fn select_df_hca() -> Command {
    Command::new(0x00, 0xA4, 0x04, 0x0C).with_data(AID_HCA)
}

/// `READ BINARY` via short file identifier; also selects the EF implicitly.
pub fn read_binary_sfi(sfi: u8, offset: u8, ne: usize) -> Command {
    Command::new(0x00, 0xB0, 0x80 | (sfi & 0x1F), offset).with_ne(ne)
}

/// `READ BINARY` on the currently selected EF with a 15-bit offset.
pub fn read_binary(offset: u16, ne: usize) -> Command {
    let [p1, p2] = (offset & MAX_OFFSET).to_be_bytes();
    Command::new(0x00, 0xB0, p1, p2).with_ne(ne)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodings() {
        assert_eq!(
            select_df_hca().to_bytes().unwrap(),
            [
                0x00, 0xA4, 0x04, 0x0C, 0x06, 0xD2, 0x76, 0x00, 0x00, 0x01, 0x02
            ]
        );
        assert_eq!(
            read_binary_sfi(0x0C, 0, 25).to_bytes().unwrap(),
            [0x00, 0xB0, 0x8C, 0x00, 0x19]
        );
        assert_eq!(
            read_binary(0x0123, 256).to_bytes().unwrap(),
            [0x00, 0xB0, 0x01, 0x23, 0x00]
        );
    }
}

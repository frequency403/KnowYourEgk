//! ISO/IEC 7816-4 APDUs and BER-TLV helpers.
//!
//! The crate is transport-agnostic: it only builds and parses byte strings.

mod command;
mod hex;
mod response;
pub mod tlv;

pub use command::{Command, Encoding, MAX_NE_EXTENDED, MAX_NE_SHORT};
pub use hex::{Hex, to_hex};
pub use response::{Response, StatusWord};

/// Errors raised while building or parsing APDUs.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApduError {
    #[error("command data too long: {0} bytes")]
    DataTooLong(usize),
    #[error("expected response length too large: {0}")]
    NeTooLarge(usize),
    #[error("command does not fit a short APDU (Nc = {nc}, Ne = {ne:?})")]
    NotShortEncodable { nc: usize, ne: Option<usize> },
    #[error("response shorter than two bytes")]
    ResponseTooShort,
    #[error("malformed TLV: {0}")]
    Tlv(&'static str),
}

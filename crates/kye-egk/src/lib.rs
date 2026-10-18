//! eGK G2.1 object model (gemSpec_eGK_ObjSys_G2.1) and VSD file formats
//! (gemSpec_eGK_Fach_VSDM).

pub mod commands;
pub mod contactless;
pub mod objects;
pub mod read;
pub mod status_vd;
pub mod vsd;

pub use status_vd::StatusVd;
pub use vsd::{RawVsd, read_raw_vsd};

/// Errors raised while reading or parsing eGK objects.
#[derive(Debug, thiserror::Error)]
pub enum EgkError {
    #[error(transparent)]
    Transport(#[from] kye_transport::TransportError),
    #[error(transparent)]
    Pace(#[from] kye_pace::PaceError),
    #[error(transparent)]
    Apdu(#[from] kye_apdu::ApduError),
    #[error("{command} failed with SW {sw}")]
    Status {
        command: &'static str,
        sw: kye_apdu::StatusWord,
    },
    #[error("file ended after {got} of {expected} bytes")]
    ShortFile { got: usize, expected: usize },
    #[error("malformed {file}: {reason}")]
    Format {
        file: &'static str,
        reason: &'static str,
    },
}

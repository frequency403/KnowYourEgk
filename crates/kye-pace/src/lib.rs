//! PACE (gemSpec_COS N085.064, ICAO 9303 Part 11) and EF.CardAccess parsing.
//!
//! Milestone M0 covers EF.CardAccess, MSE:Set AT and the first General
//! Authenticate step. The key agreement itself follows in M2.

pub mod apdu;
pub mod card_access;
pub mod oid;

pub use card_access::{CardAccess, PaceInfo, SecurityInfo};
pub use oid::{Oid, PaceProtocol, StandardizedDomainParameters};

/// Errors raised while running or parsing PACE.
#[derive(Debug, thiserror::Error)]
pub enum PaceError {
    #[error(transparent)]
    Transport(#[from] kye_transport::TransportError),
    #[error(transparent)]
    Apdu(#[from] kye_apdu::ApduError),
    #[error("{step} failed with SW {sw}")]
    Status {
        step: &'static str,
        sw: kye_apdu::StatusWord,
    },
    #[error("malformed EF.CardAccess: {0}")]
    CardAccess(&'static str),
    #[error("malformed General Authenticate response: {0}")]
    GaResponse(&'static str),
    #[error("no supported PACE protocol in EF.CardAccess")]
    Unsupported,
}

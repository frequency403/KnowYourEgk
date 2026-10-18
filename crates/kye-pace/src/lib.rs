//! PACE (gemSpec_COS N085.064, ICAO 9303 Part 11) and EF.CardAccess parsing.
//!
//! Supported: id-PACE-ECDH-GM-AES-CBC-CMAC-128 on brainpoolP256r1, the
//! protocol announced by the eGK G2.1 in EF.CardAccess.

pub mod apdu;
pub mod card_access;
pub mod crypto;
pub mod oid;
mod pace;

pub use card_access::{CardAccess, PaceInfo, SecurityInfo};
pub use crypto::{Password, SessionKeys};
pub use oid::{Oid, PaceProtocol, StandardizedDomainParameters};
pub use pace::{KeySource, OsKeys, PaceSession, establish, establish_with};

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
    #[error("CAN must be exactly six digits")]
    InvalidPassword,
    #[error("wrong password (retries left: {retries:?})")]
    WrongPassword { retries: Option<u8> },
    #[error("invalid or degenerate elliptic curve point")]
    InvalidPoint,
    #[error("chip authentication token T_PICC does not match")]
    TokenMismatch,
    #[error("system random number generator failed")]
    Rng,
}

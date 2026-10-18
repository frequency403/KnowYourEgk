//! End-to-end VSD read over either interface.

use kye_apdu::StatusWord;
use kye_pace::Password;
use kye_transport::{CardInterface, CardTransport};

use crate::EgkError;
use crate::contactless::open_secure_channel;
use crate::vsd::{RawVsd, read_raw_vsd};

/// Plaintext bytes per READ BINARY.
const CHUNK: usize = 256;

/// Progress reported by [`read_vsd`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Reading,
    Pace,
    ReadingSecure,
}

/// Raw VSD plus how it was obtained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadResult {
    /// `true` if PACE + secure messaging was used.
    pub secured: bool,
    pub raw: RawVsd,
}

/// Reads EF.StatusVD/PD/VD from the connected card.
///
/// Contactless cards (by ATR) go through PACE with `can` straight away.
/// Contact cards are read in plain; if the card answers `69 82` (access
/// requires PACE), the read is retried over PACE as a fallback.
pub fn read_vsd<T: CardTransport + ?Sized>(
    t: &mut T,
    can: Option<&Password>,
    progress: &mut dyn FnMut(Step),
) -> Result<ReadResult, EgkError> {
    if t.interface() == CardInterface::Contactless {
        return read_secure(t, can, progress);
    }
    progress(Step::Reading);
    match read_raw_vsd(t, CHUNK) {
        Ok(raw) => Ok(ReadResult {
            secured: false,
            raw,
        }),
        Err(EgkError::Status { sw, .. }) if sw == StatusWord::SECURITY_STATUS_NOT_SATISFIED => {
            read_secure(t, can, progress)
        }
        Err(e) => Err(e),
    }
}

fn read_secure<T: CardTransport + ?Sized>(
    t: &mut T,
    can: Option<&Password>,
    progress: &mut dyn FnMut(Step),
) -> Result<ReadResult, EgkError> {
    let can = can.ok_or(EgkError::CanRequired)?;
    progress(Step::Pace);
    let mut sm = open_secure_channel(t, can)?;
    progress(Step::ReadingSecure);
    let raw = read_raw_vsd(&mut sm, CHUNK)?;
    Ok(ReadResult { secured: true, raw })
}

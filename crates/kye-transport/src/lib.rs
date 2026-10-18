//! Card transport abstraction.
//!
//! Everything above this crate talks to a card through [`CardTransport`]; the
//! concrete backend (PC/SC today, NFC on mobile later) is chosen by the app.
//! All calls are blocking and meant to run on a dedicated worker thread.

pub mod atr;
pub mod mock;
#[cfg(feature = "pcsc")]
pub mod pcsc;
mod tracing_transport;

use kye_apdu::{ApduError, Command, Response};

pub use atr::CardInterface;
pub use tracing_transport::TracingTransport;

/// Errors raised by transports and transport decorators.
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("no card present")]
    NoCard,
    #[error("card removed or reset")]
    CardRemoved,
    #[error("reader not found: {0}")]
    UnknownReader(String),
    #[error("smart card service not available")]
    ServiceUnavailable,
    #[error("transport timeout")]
    Timeout,
    #[error(transparent)]
    Apdu(#[from] ApduError),
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("backend error: {0}")]
    Backend(String),
}

/// A connection to a single card that can exchange APDUs.
pub trait CardTransport {
    /// Sends one command and returns the card's response.
    ///
    /// Implementations handle transport-level details (`61 XX` / `6C XX` on
    /// T=0); the returned status word is the card's final answer.
    fn transmit(&mut self, cmd: &Command) -> Result<Response, TransportError>;

    /// ATR (contact) or PC/SC pseudo-ATR (contactless) of the connected card.
    fn atr(&self) -> &[u8];

    /// Interface the card is connected through, derived from the ATR.
    fn interface(&self) -> CardInterface {
        CardInterface::from_atr(self.atr())
    }
}

impl<T: CardTransport + ?Sized> CardTransport for &mut T {
    fn transmit(&mut self, cmd: &Command) -> Result<Response, TransportError> {
        (**self).transmit(cmd)
    }

    fn atr(&self) -> &[u8] {
        (**self).atr()
    }
}

impl<T: CardTransport + ?Sized> CardTransport for Box<T> {
    fn transmit(&mut self, cmd: &Command) -> Result<Response, TransportError> {
        (**self).transmit(cmd)
    }

    fn atr(&self) -> &[u8] {
        (**self).atr()
    }
}

/// Snapshot of one reader slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReaderInfo {
    pub name: String,
    /// ATR of the present card, `None` if the slot is empty.
    pub atr: Option<Vec<u8>>,
}

impl ReaderInfo {
    pub fn interface(&self) -> Option<CardInterface> {
        self.atr.as_deref().map(CardInterface::from_atr)
    }
}

/// Enumerates readers and opens connections to cards.
pub trait ReaderProvider {
    type Transport: CardTransport;

    /// Lists all reader slots together with the ATR of any inserted card.
    fn readers(&self) -> Result<Vec<ReaderInfo>, TransportError>;

    /// Opens an exclusive connection to the card in `reader`.
    fn connect(&self, reader: &str) -> Result<Self::Transport, TransportError>;
}

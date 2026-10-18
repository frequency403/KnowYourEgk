//! PC/SC backend (winscard on Windows, PCSC.framework on macOS, pcsc-lite on Linux).

use std::ffi::{CStr, CString};
use std::time::Duration;

use kye_apdu::{Command, Response, StatusWord};
use pcsc::{
    Card, Context, Disposition, MAX_BUFFER_SIZE_EXTENDED, Protocols, ReaderState, Scope, ShareMode,
    State, Transaction,
};

use crate::{CardTransport, ReaderInfo, ReaderProvider, TransportError};

/// Upper bound for GET RESPONSE rounds, guards against misbehaving cards.
const MAX_GET_RESPONSE_ROUNDS: usize = 64;

impl From<pcsc::Error> for TransportError {
    fn from(e: pcsc::Error) -> Self {
        match e {
            // pcsc-lite reports an IFD communication error in SCardTransmit as
            // SCARD_E_NOT_TRANSACTED, e.g. when a card leaves the RF field.
            pcsc::Error::NoSmartcard => Self::NoCard,
            pcsc::Error::RemovedCard
            | pcsc::Error::ResetCard
            | pcsc::Error::NotTransacted
            | pcsc::Error::UnpoweredCard
            | pcsc::Error::UnresponsiveCard => Self::CardRemoved,
            pcsc::Error::Timeout => Self::Timeout,
            pcsc::Error::UnknownReader | pcsc::Error::ReaderUnavailable => {
                Self::UnknownReader(e.to_string())
            }
            // Windows stops the smart card service when the last reader is
            // unplugged; pcsc-lite may be socket-activated and not running yet.
            pcsc::Error::NoService | pcsc::Error::ServiceStopped => Self::ServiceUnavailable,
            other => Self::Backend(other.to_string()),
        }
    }
}

/// PC/SC reader provider.
pub struct PcscProvider {
    ctx: Context,
}

impl PcscProvider {
    pub fn new() -> Result<Self, TransportError> {
        Ok(Self {
            ctx: Context::establish(Scope::User)?,
        })
    }

    fn reader_names(&self) -> Result<Vec<CString>, TransportError> {
        match self.ctx.list_readers_owned() {
            Ok(names) => Ok(names),
            Err(pcsc::Error::NoReadersAvailable) => Ok(Vec::new()),
            Err(e) => Err(e.into()),
        }
    }
}

impl ReaderProvider for PcscProvider {
    type Transport = PcscCard;

    fn readers(&self) -> Result<Vec<ReaderInfo>, TransportError> {
        let names = self.reader_names()?;
        if names.is_empty() {
            return Ok(Vec::new());
        }
        let mut states: Vec<_> = names
            .into_iter()
            .map(|n| ReaderState::new(n, State::UNAWARE))
            .collect();
        self.ctx.get_status_change(Duration::ZERO, &mut states)?;
        Ok(states
            .iter()
            .map(|s| {
                let present = s.event_state().contains(State::PRESENT)
                    && !s.event_state().contains(State::MUTE);
                ReaderInfo {
                    name: s.name().to_string_lossy().into_owned(),
                    atr: (present && !s.atr().is_empty()).then(|| s.atr().to_vec()),
                }
            })
            .collect())
    }

    fn connect(&self, reader: &str) -> Result<PcscCard, TransportError> {
        let name =
            CString::new(reader).map_err(|_| TransportError::UnknownReader(reader.into()))?;
        PcscCard::connect(&self.ctx, &name)
    }
}

/// An exclusive connection to a card.
pub struct PcscCard {
    card: Card,
    atr: Vec<u8>,
}

impl PcscCard {
    fn connect(ctx: &Context, reader: &CStr) -> Result<Self, TransportError> {
        let card = ctx.connect(reader, ShareMode::Exclusive, Protocols::ANY)?;
        let atr = card.status2_owned()?.atr().to_vec();
        Ok(Self { card, atr })
    }

    /// Starts a PC/SC transaction (`SCardBeginTransaction`).
    ///
    /// Run PACE and all following reads inside one transaction so that no
    /// other process (e.g. Windows Smart Card PnP) can interleave APDUs.
    pub fn transaction(&mut self) -> Result<PcscTransaction<'_>, TransportError> {
        Ok(PcscTransaction {
            tx: Some(self.card.transaction()?),
            atr: &self.atr,
        })
    }

    /// Disconnects and resets the card, dropping any security state.
    pub fn disconnect_reset(self) -> Result<(), TransportError> {
        self.card
            .disconnect(Disposition::ResetCard)
            .map_err(|(_, e)| e.into())
    }
}

impl CardTransport for PcscCard {
    fn transmit(&mut self, cmd: &Command) -> Result<Response, TransportError> {
        transmit(&self.card, cmd)
    }

    fn atr(&self) -> &[u8] {
        &self.atr
    }
}

/// A card handle inside an active PC/SC transaction.
///
/// The transaction ends with `SCardEndTransaction(SCARD_LEAVE_CARD)` on drop,
/// or explicitly via [`PcscTransaction::end`].
pub struct PcscTransaction<'a> {
    tx: Option<Transaction<'a>>,
    atr: &'a [u8],
}

impl PcscTransaction<'_> {
    /// Ends the transaction with the given card disposition.
    pub fn end(mut self, reset: bool) -> Result<(), TransportError> {
        let disposition = if reset {
            Disposition::ResetCard
        } else {
            Disposition::LeaveCard
        };
        match self.tx.take() {
            Some(tx) => tx.end(disposition).map_err(|(_, e)| e.into()),
            None => Ok(()),
        }
    }
}

impl CardTransport for PcscTransaction<'_> {
    fn transmit(&mut self, cmd: &Command) -> Result<Response, TransportError> {
        let tx = self
            .tx
            .as_ref()
            .ok_or_else(|| TransportError::Protocol("transaction already ended".into()))?;
        transmit(tx, cmd)
    }

    fn atr(&self) -> &[u8] {
        self.atr
    }
}

fn transmit_raw(card: &Card, apdu: &[u8]) -> Result<Response, TransportError> {
    let mut buf = vec![0u8; MAX_BUFFER_SIZE_EXTENDED];
    let rsp = card.transmit(apdu, &mut buf)?;
    Ok(Response::from_bytes(rsp)?)
}

fn transmit(card: &Card, cmd: &Command) -> Result<Response, TransportError> {
    let mut rsp = transmit_raw(card, &cmd.to_bytes()?)?;

    // 6C XX: wrong Le, resend with the length the card asked for (T=0).
    if rsp.sw.sw1() == 0x6C {
        let ne = match rsp.sw.sw2() {
            0 => 256,
            n => usize::from(n),
        };
        let retry = cmd.clone().with_ne(ne);
        rsp = transmit_raw(card, &retry.to_bytes()?)?;
    }

    // 61 XX: collect remaining bytes with GET RESPONSE (T=0).
    let mut data = std::mem::take(&mut rsp.data);
    let mut sw = rsp.sw;
    let mut rounds = 0;
    while let Some(remaining) = sw.bytes_remaining() {
        rounds += 1;
        if rounds > MAX_GET_RESPONSE_ROUNDS {
            return Err(TransportError::Protocol(
                "too many GET RESPONSE rounds".into(),
            ));
        }
        let ne = if remaining == 0 {
            256
        } else {
            usize::from(remaining)
        };
        let get = Command::new(cmd.cla & 0x0F, 0xC0, 0x00, 0x00).with_ne(ne);
        let part = transmit_raw(card, &get.to_bytes()?)?;
        data.extend_from_slice(&part.data);
        sw = part.sw;
    }
    Ok(Response::new(data, StatusWord(sw.0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lost_card_errors_map_to_removed() {
        for e in [
            pcsc::Error::RemovedCard,
            pcsc::Error::ResetCard,
            pcsc::Error::NotTransacted,
            pcsc::Error::UnresponsiveCard,
        ] {
            assert!(
                matches!(TransportError::from(e), TransportError::CardRemoved),
                "{e:?}"
            );
        }
        assert!(matches!(
            TransportError::from(pcsc::Error::ServiceStopped),
            TransportError::ServiceUnavailable
        ));
    }
}

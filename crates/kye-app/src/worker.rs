//! Card worker thread: owns the PC/SC context, watches all reader slots and
//! performs blocking reads. The UI talks to it through channels only.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use kye_egk::contactless::open_secure_channel;
use kye_egk::{EgkError, Vsd, read_raw_vsd};
use kye_pace::{PaceError, Password};
use kye_transport::pcsc::PcscProvider;
use kye_transport::{CardInterface, ReaderInfo, ReaderProvider, TransportError};

const POLL: Duration = Duration::from_millis(500);
const CHUNK: usize = 256;

pub enum Request {
    Read {
        reader: String,
        can: Option<Password>,
    },
}

pub enum Event {
    Readers(Vec<ReaderInfo>),
    Progress(&'static str),
    Finished(Box<Result<ReadOutcome, ReadError>>),
    /// PC/SC is unavailable (no service, no permission); the worker retries.
    Unavailable(String),
}

pub struct ReadOutcome {
    pub interface: CardInterface,
    pub vsd: Vsd,
}

/// Read failures, phrased for the user.
#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error("Für kontaktloses Lesen wird die 6-stellige CAN benötigt.")]
    CanRequired,
    #[error("Die CAN ist falsch. Bitte mit der Nummer auf der Kartenvorderseite vergleichen.")]
    WrongCan,
    #[error("Die Karte wurde entfernt oder hat die Verbindung verloren.")]
    CardRemoved,
    #[error("Keine Karte im Leser.")]
    NoCard,
    #[error("Die Karte unterstützt kein bekanntes PACE-Verfahren.")]
    Unsupported,
    #[error("Kartenfehler: {0}")]
    Card(String),
}

impl From<TransportError> for ReadError {
    fn from(e: TransportError) -> Self {
        match e {
            TransportError::CardRemoved => Self::CardRemoved,
            TransportError::NoCard => Self::NoCard,
            other => Self::Card(other.to_string()),
        }
    }
}

impl From<PaceError> for ReadError {
    fn from(e: PaceError) -> Self {
        match e {
            // A wrong CAN shows up as a failed GA step or a T_PICC mismatch.
            // The chip rejects T_PCD in step 4 if the CAN was wrong.
            PaceError::WrongPassword { .. } | PaceError::TokenMismatch => Self::WrongCan,
            PaceError::Status { step, .. } if step.starts_with("GA4") => Self::WrongCan,
            PaceError::Unsupported => Self::Unsupported,
            PaceError::Transport(t) => t.into(),
            other => Self::Card(other.to_string()),
        }
    }
}

impl From<EgkError> for ReadError {
    fn from(e: EgkError) -> Self {
        match e {
            EgkError::Transport(t) => t.into(),
            EgkError::Pace(p) => p.into(),
            other => Self::Card(other.to_string()),
        }
    }
}

pub struct Worker {
    pub requests: Sender<Request>,
    pub events: Receiver<Event>,
}

/// Starts the worker; `notify` is called after every event (UI repaint).
pub fn spawn(notify: impl Fn() + Send + 'static) -> std::io::Result<Worker> {
    let (req_tx, req_rx) = mpsc::channel();
    let (ev_tx, ev_rx) = mpsc::channel();
    thread::Builder::new()
        .name("card-worker".into())
        .spawn(move || run(&req_rx, &ev_tx, &notify))?;
    Ok(Worker {
        requests: req_tx,
        events: ev_rx,
    })
}

fn run(requests: &Receiver<Request>, events: &Sender<Event>, notify: &dyn Fn()) {
    let send = |e: Event| {
        let ok = events.send(e).is_ok();
        notify();
        ok
    };
    let mut provider: Option<PcscProvider> = None;
    let mut known: Option<Vec<ReaderInfo>> = None;

    loop {
        if provider.is_none() {
            match PcscProvider::new() {
                Ok(p) => provider = Some(p),
                Err(e) => {
                    if !send(Event::Unavailable(e.to_string())) {
                        return;
                    }
                }
            }
        }

        if let Some(p) = &provider {
            match p.readers() {
                Ok(readers) if known.as_ref() != Some(&readers) => {
                    known = Some(readers.clone());
                    if !send(Event::Readers(readers)) {
                        return;
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    // The context may become invalid (service restart); re-establish it.
                    provider = None;
                    known = None;
                    if !send(Event::Unavailable(e.to_string())) {
                        return;
                    }
                }
            }
        }

        match requests.recv_timeout(POLL) {
            Ok(Request::Read { reader, can }) => {
                let result = match &provider {
                    Some(p) => read(p, &reader, can.as_ref(), &|s| {
                        send(Event::Progress(s));
                    }),
                    None => Err(ReadError::Card("PC/SC nicht verfügbar".into())),
                };
                if !send(Event::Finished(Box::new(result))) {
                    return;
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn read(
    provider: &PcscProvider,
    reader: &str,
    can: Option<&Password>,
    progress: &dyn Fn(&'static str),
) -> Result<ReadOutcome, ReadError> {
    progress("Verbinde mit der Karte …");
    let mut card = provider.connect(reader)?;
    let interface = kye_transport::CardTransport::interface(&card);
    let mut tx = card.transaction()?;

    let raw = match interface {
        CardInterface::Contactless => {
            let can = can.ok_or(ReadError::CanRequired)?;
            progress("PACE mit CAN …");
            let mut sm = open_secure_channel(&mut tx, can)?;
            progress("Lese Versichertendaten (verschlüsselt) …");
            read_raw_vsd(&mut sm, CHUNK)?
        }
        _ => {
            progress("Lese Versichertendaten …");
            match read_raw_vsd(&mut tx, CHUNK) {
                // Fallback: access requires PACE although the ATR said contact.
                Err(EgkError::Status { sw, .. })
                    if sw == kye_apdu::StatusWord::SECURITY_STATUS_NOT_SATISFIED =>
                {
                    let can = can.ok_or(ReadError::CanRequired)?;
                    progress("PACE mit CAN …");
                    let mut sm = open_secure_channel(&mut tx, can)?;
                    read_raw_vsd(&mut sm, CHUNK)?
                }
                other => other?,
            }
        }
    };
    // Reset the card so no security state outlives this read.
    tx.end(true)?;

    progress("Werte Daten aus …");
    let vsd = Vsd::parse(&raw)?;
    Ok(ReadOutcome { interface, vsd })
}

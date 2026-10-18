//! Card worker thread: owns the PC/SC context, watches all reader slots and
//! performs blocking reads. The UI talks to it through channels only.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use kye_egk::session::{Step, read_vsd};
use kye_egk::{EgkError, Vsd};
use kye_pace::{PaceError, Password};
use kye_transport::pcsc::PcscProvider;
use kye_transport::{CardInterface, ReaderInfo, ReaderProvider, TransportError};

const POLL: Duration = Duration::from_millis(500);

pub enum Request {
    Read {
        reader: String,
        can: Option<Password>,
    },
}

/// Steps of a read, translated by the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    Starting,
    Connecting,
    Reading,
    Pace,
    ReadingSecure,
    Parsing,
}

pub enum Event {
    Readers(Vec<ReaderInfo>),
    Progress(Progress),
    Finished(Box<Result<ReadOutcome, ReadError>>),
    /// PC/SC is unavailable (no service, no permission); the worker retries.
    Unavailable(String),
}

pub struct ReadOutcome {
    pub interface: CardInterface,
    /// Read over PACE + secure messaging.
    pub secured: bool,
    pub vsd: Vsd,
}

/// Read failures. The UI shows them translated; `Display` is for logs.
#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error("CAN required")]
    CanRequired,
    #[error("wrong CAN")]
    WrongCan,
    #[error("card removed")]
    CardRemoved,
    #[error("reader gone")]
    ReaderGone,
    #[error("no card")]
    NoCard,
    #[error("no supported PACE protocol")]
    Unsupported,
    #[error("PC/SC service unavailable")]
    ServiceUnavailable,
    /// Technical detail from the lower layers, not translated.
    #[error("card error: {0}")]
    Card(String),
}

impl From<TransportError> for ReadError {
    fn from(e: TransportError) -> Self {
        match e {
            TransportError::CardRemoved => Self::CardRemoved,
            TransportError::NoCard => Self::NoCard,
            TransportError::UnknownReader(_) | TransportError::ServiceUnavailable => {
                Self::ReaderGone
            }
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
            EgkError::CanRequired => Self::CanRequired,
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
    // Last state sent to the UI, so unchanged polls send nothing.
    let mut shown: Option<Result<Vec<ReaderInfo>, String>> = None;

    loop {
        if provider.is_none() {
            match PcscProvider::new() {
                Ok(p) => provider = Some(p),
                // No service (e.g. Windows after the last reader was unplugged)
                // simply means: no readers. Retry on the next poll.
                Err(TransportError::ServiceUnavailable) => {}
                Err(e) => {
                    let state = Err(e.to_string());
                    if shown.as_ref() != Some(&state) {
                        if !send(Event::Unavailable(e.to_string())) {
                            return;
                        }
                        shown = Some(state);
                    }
                }
            }
        }

        let readers = match &provider {
            Some(p) => match p.readers() {
                Ok(r) => Some(r),
                // A reader vanished between listing and querying it: transient
                // during hot-plug, try again on the next poll.
                Err(TransportError::UnknownReader(_) | TransportError::Timeout) => None,
                // Context became invalid (service stopped/restarted): re-establish.
                Err(_) => {
                    provider = None;
                    Some(Vec::new())
                }
            },
            None if !matches!(shown, Some(Err(_))) => Some(Vec::new()),
            None => None,
        };
        if let Some(r) = readers {
            let state = Ok(r);
            if shown.as_ref() != Some(&state) {
                if let Ok(r) = &state
                    && !send(Event::Readers(r.clone()))
                {
                    return;
                }
                shown = Some(state);
            }
        }

        match requests.recv_timeout(POLL) {
            Ok(Request::Read { reader, can }) => {
                let result = match &provider {
                    Some(p) => read(p, &reader, can.as_ref(), &|s| {
                        send(Event::Progress(s));
                    }),
                    None => Err(ReadError::ServiceUnavailable),
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
    progress: &dyn Fn(Progress),
) -> Result<ReadOutcome, ReadError> {
    progress(Progress::Connecting);
    let mut card = provider.connect(reader)?;
    let interface = kye_transport::CardTransport::interface(&card);
    let mut tx = card.transaction()?;

    let result = read_vsd(&mut tx, can, &mut |step| {
        progress(match step {
            Step::Reading => Progress::Reading,
            Step::Pace => Progress::Pace,
            Step::ReadingSecure => Progress::ReadingSecure,
        })
    });
    // Reset the card so no security state outlives this read, also on errors.
    let ended = tx.end(true);
    let result = result?;
    ended?;

    progress(Progress::Parsing);
    let vsd = Vsd::parse(&result.raw)?;
    Ok(ReadOutcome {
        interface,
        secured: result.secured,
        vsd,
    })
}

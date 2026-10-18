//! Scripted transport for tests: replays a fixed sequence of exchanges.

use std::collections::VecDeque;

use kye_apdu::{Command, Hex, Response};

use crate::{CardTransport, TransportError};

#[derive(Debug, Clone)]
enum Reply {
    Bytes(Vec<u8>),
    Removed,
}

#[derive(Debug, Clone)]
struct Exchange {
    command: Vec<u8>,
    /// Match only the leading bytes (for commands with random content).
    prefix: bool,
    reply: Reply,
}

/// A [`CardTransport`] that checks each command against a script.
///
/// Any deviation from the script yields [`TransportError::Protocol`].
#[derive(Debug, Clone)]
pub struct MockTransport {
    atr: Vec<u8>,
    script: VecDeque<Exchange>,
}

impl MockTransport {
    pub fn new(atr: impl Into<Vec<u8>>) -> Self {
        Self {
            atr: atr.into(),
            script: VecDeque::new(),
        }
    }

    /// Expects `command` (serialised APDU) and answers with `response` (data || SW).
    pub fn expect(mut self, command: impl Into<Vec<u8>>, response: impl Into<Vec<u8>>) -> Self {
        self.script.push_back(Exchange {
            command: command.into(),
            prefix: false,
            reply: Reply::Bytes(response.into()),
        });
        self
    }

    /// Expects a command starting with `prefix` and answers with `response`.
    pub fn expect_prefix(
        mut self,
        prefix: impl Into<Vec<u8>>,
        response: impl Into<Vec<u8>>,
    ) -> Self {
        self.script.push_back(Exchange {
            command: prefix.into(),
            prefix: true,
            reply: Reply::Bytes(response.into()),
        });
        self
    }

    /// Expects `command` and simulates card removal.
    pub fn expect_removed(mut self, command: impl Into<Vec<u8>>) -> Self {
        self.script.push_back(Exchange {
            command: command.into(),
            prefix: false,
            reply: Reply::Removed,
        });
        self
    }

    /// Whether every scripted exchange has been consumed.
    pub fn is_done(&self) -> bool {
        self.script.is_empty()
    }
}

impl CardTransport for MockTransport {
    fn transmit(&mut self, cmd: &Command) -> Result<Response, TransportError> {
        let bytes = cmd.to_bytes()?;
        let ex = self.script.pop_front().ok_or_else(|| {
            TransportError::Protocol(format!("unexpected command {}", Hex(&bytes)))
        })?;
        let matches = if ex.prefix {
            bytes.starts_with(&ex.command)
        } else {
            ex.command == bytes
        };
        if !matches {
            return Err(TransportError::Protocol(format!(
                "expected {}, got {}",
                Hex(&ex.command),
                Hex(&bytes)
            )));
        }
        match ex.reply {
            Reply::Bytes(raw) => Ok(Response::from_bytes(&raw)?),
            Reply::Removed => Err(TransportError::CardRemoved),
        }
    }

    fn atr(&self) -> &[u8] {
        &self.atr
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replays_script() {
        let mut t = MockTransport::new([0x3B])
            .expect([0x00, 0xA4, 0x00, 0x0C], [0x90, 0x00])
            .expect_removed([0x00, 0xB0, 0x00, 0x00, 0x00]);
        let r = t.transmit(&Command::new(0x00, 0xA4, 0x00, 0x0C)).unwrap();
        assert!(r.sw.is_ok());
        let e = t.transmit(&Command::new(0x00, 0xB0, 0, 0).with_ne(256));
        assert!(matches!(e, Err(TransportError::CardRemoved)));
        assert!(t.is_done());
        assert!(matches!(
            t.transmit(&Command::new(0, 0, 0, 0)),
            Err(TransportError::Protocol(_))
        ));
    }
}

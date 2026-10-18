use kye_apdu::{Command, Response};
use tracing::debug;

use crate::{CardTransport, TransportError};

/// Logs every exchange at `debug` level.
///
/// By default only header, Lc, Le and status word are logged. Command and
/// response data are logged only with the `trace-payload` feature, because
/// they may contain personal data or session keys.
pub struct TracingTransport<T> {
    inner: T,
    label: &'static str,
}

impl<T> TracingTransport<T> {
    pub fn new(inner: T, label: &'static str) -> Self {
        Self { inner, label }
    }

    pub fn into_inner(self) -> T {
        self.inner
    }

    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T: CardTransport> CardTransport for TracingTransport<T> {
    fn transmit(&mut self, cmd: &Command) -> Result<Response, TransportError> {
        let [cla, ins, p1, p2] = cmd.header();
        let ext = cmd.is_extended().unwrap_or(false);
        debug!(
            target: "kye::apdu",
            "{} > {cla:02X} {ins:02X} {p1:02X} {p2:02X} Lc={} Le={:?}{}",
            self.label,
            cmd.data.len(),
            cmd.ne,
            if ext { " (ext)" } else { "" },
        );
        #[cfg(feature = "trace-payload")]
        debug!(target: "kye::apdu", "{} > data {}", self.label, kye_apdu::Hex(&cmd.data));

        let result = self.inner.transmit(cmd);
        match &result {
            Ok(rsp) => {
                debug!(
                    target: "kye::apdu",
                    "{} < SW={} len={}",
                    self.label,
                    rsp.sw,
                    rsp.data.len()
                );
                #[cfg(feature = "trace-payload")]
                debug!(target: "kye::apdu", "{} < data {}", self.label, kye_apdu::Hex(&rsp.data));
            }
            Err(e) => debug!(target: "kye::apdu", "{} < error: {e}", self.label),
        }
        result
    }

    fn atr(&self) -> &[u8] {
        self.inner.atr()
    }
}

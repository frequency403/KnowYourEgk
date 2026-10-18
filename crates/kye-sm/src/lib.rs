//! Secure messaging (gemSpec_COS 13.2/13.3, ICAO 9303-11 9.8) for AES-128
//! session keys from PACE, as a [`CardTransport`] decorator.
//!
//! Command: `CLA|0C INS P1 P2 Lc [87 L 01 C] [97 L Le] 8E 08 MAC Le`
//! Response: `[87 L 01 C | 81 L D] 99 02 SW 8E 08 MAC SW`
//!
//! - SSC starts at 0 and is incremented before every command and every response.
//! - `IV = AES-ENC(K_enc, SSC)`, AES-CBC with ISO/IEC 9797-1 padding method 2.
//! - `MAC = CMAC(K_mac, Pad(SSC || Pad(header) || DOs))[0..8]`; the header is
//!   not padded if no data objects follow.
//! - Secured case 2/4 commands are sent as case 4 extended with Le `00 00`,
//!   case 1/3 as case 4 short with Le `00`.
//!
//! Any SM failure (missing or wrong MAC, plain error response) closes the
//! channel; all further calls fail with [`SmError::Closed`].

mod crypto;

use kye_apdu::{Command, Encoding, MAX_NE_EXTENDED, MAX_NE_SHORT, Response, StatusWord, tlv};
use kye_transport::{CardTransport, TransportError};
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::crypto::{cbc_decrypt, cbc_encrypt, cmac8, iv_from_ssc, pad, unpad};

const TAG_CRYPTOGRAM: u32 = 0x87;
const TAG_PLAIN: u32 = 0x81;
const TAG_LE: u32 = 0x97;
const TAG_STATUS: u32 = 0x99;
const TAG_MAC: u32 = 0x8E;
/// Padding-content indicator for DO87: ISO/IEC 9797-1 method 2.
const PADDING_INDICATOR: u8 = 0x01;

/// Secure messaging errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SmError {
    #[error("card answered without secure messaging (SW {0})")]
    PlainResponse(StatusWord),
    #[error("response MAC is missing or wrong")]
    MacMismatch,
    #[error("malformed secure messaging response: {0}")]
    Malformed(&'static str),
    #[error("secure channel closed after an earlier error")]
    Closed,
}

impl From<SmError> for TransportError {
    fn from(e: SmError) -> Self {
        TransportError::Protocol(e.to_string())
    }
}

#[derive(Zeroize, ZeroizeOnDrop)]
struct State {
    k_enc: [u8; 16],
    k_mac: [u8; 16],
    ssc: [u8; 16],
}

impl State {
    fn increment_ssc(&mut self) {
        for b in self.ssc.iter_mut().rev() {
            let (v, carry) = b.overflowing_add(1);
            *b = v;
            if !carry {
                break;
            }
        }
    }
}

/// A [`CardTransport`] that protects every command with secure messaging.
pub struct SecureChannel<T> {
    inner: T,
    state: State,
    closed: bool,
}

impl<T: CardTransport> SecureChannel<T> {
    /// Opens a channel with fresh PACE session keys (SSC = 0).
    pub fn new(inner: T, k_enc: &[u8; 16], k_mac: &[u8; 16]) -> Self {
        Self {
            inner,
            state: State {
                k_enc: *k_enc,
                k_mac: *k_mac,
                ssc: [0; 16],
            },
            closed: false,
        }
    }

    pub fn into_inner(self) -> T {
        // `State` zeroizes itself on drop.
        let Self { inner, .. } = self;
        inner
    }

    /// Builds the protected command and advances the SSC.
    fn wrap(&mut self, cmd: &Command) -> Result<Command, TransportError> {
        let header_cla = cmd.cla | 0x0C;
        self.state.increment_ssc();

        let mut dos = Vec::new();
        if !cmd.data.is_empty() {
            let iv = iv_from_ssc(&self.state.k_enc, &self.state.ssc);
            let mut cryptogram = vec![PADDING_INDICATOR];
            cryptogram.extend_from_slice(&cbc_encrypt(&self.state.k_enc, &iv, &pad(&cmd.data)));
            tlv::write(&mut dos, TAG_CRYPTOGRAM, &cryptogram);
        }
        if let Some(ne) = cmd.ne {
            let le: &[u8] = match ne {
                MAX_NE_EXTENDED => &[0x00, 0x00],
                n if n > MAX_NE_SHORT => &[(n >> 8) as u8, n as u8],
                n => &[n as u8],
            };
            tlv::write(&mut dos, TAG_LE, le);
        }

        let header = [header_cla, cmd.ins, cmd.p1, cmd.p2];
        let mut mac_input = self.state.ssc.to_vec();
        if dos.is_empty() {
            mac_input.extend_from_slice(&header);
        } else {
            mac_input.extend_from_slice(&pad(&header));
            mac_input.extend_from_slice(&dos);
        }
        let mac = cmac8(&self.state.k_mac, &pad(&mac_input));
        tlv::write(&mut dos, TAG_MAC, &mac);

        let protected = Command::new(header_cla, cmd.ins, cmd.p1, cmd.p2).with_data(dos);
        Ok(if cmd.ne.is_some() {
            protected
                .with_ne(MAX_NE_EXTENDED)
                .with_encoding(Encoding::Extended)
        } else {
            protected
                .with_ne(MAX_NE_SHORT)
                .with_encoding(Encoding::Short)
        })
    }

    /// Verifies and decrypts a protected response and advances the SSC.
    fn unwrap(&mut self, rsp: &Response) -> Result<Response, SmError> {
        self.state.increment_ssc();
        if rsp.data.is_empty() {
            return Err(SmError::PlainResponse(rsp.sw));
        }

        let mut body: Option<(u32, &[u8])> = None;
        let mut status: Option<&[u8]> = None;
        let mut mac: Option<&[u8]> = None;
        let mut mac_covered_end = 0;
        let mut offset = 0;
        for obj in tlv::TlvIter::new(&rsp.data) {
            let obj = obj.map_err(|_| SmError::Malformed("bad TLV"))?;
            let encoded_len = tlv::encode(obj.tag, obj.value).len();
            match obj.tag {
                TAG_CRYPTOGRAM | TAG_PLAIN if body.is_none() && status.is_none() => {
                    body = Some((obj.tag, obj.value));
                }
                TAG_STATUS if status.is_none() => status = Some(obj.value),
                TAG_MAC if mac.is_none() => {
                    mac = Some(obj.value);
                    mac_covered_end = offset;
                }
                _ => return Err(SmError::Malformed("unexpected data object")),
            }
            offset += encoded_len;
        }

        let mac = mac.ok_or(SmError::MacMismatch)?;
        if mac_covered_end + 10 != rsp.data.len() {
            return Err(SmError::Malformed("MAC is not the last data object"));
        }
        let mut mac_input = self.state.ssc.to_vec();
        mac_input.extend_from_slice(&rsp.data[..mac_covered_end]);
        let expected = cmac8(&self.state.k_mac, &pad(&mac_input));
        if mac.len() != 8 || !bool::from(subtle::ConstantTimeEq::ct_eq(mac, &expected[..])) {
            return Err(SmError::MacMismatch);
        }

        let sw = match status {
            Some([sw1, sw2]) => StatusWord::from_bytes(*sw1, *sw2),
            _ => return Err(SmError::Malformed("missing status DO99")),
        };
        let data = match body {
            None => Vec::new(),
            Some((TAG_PLAIN, value)) => value.to_vec(),
            Some((_, value)) => {
                let (&indicator, cryptogram) = value
                    .split_first()
                    .ok_or(SmError::Malformed("empty DO87"))?;
                if indicator != PADDING_INDICATOR
                    || cryptogram.is_empty()
                    || !cryptogram.len().is_multiple_of(16)
                {
                    return Err(SmError::Malformed("bad DO87 cryptogram"));
                }
                let iv = iv_from_ssc(&self.state.k_enc, &self.state.ssc);
                let mut plain = cbc_decrypt(&self.state.k_enc, &iv, cryptogram);
                let data = unpad(&plain).map(<[u8]>::to_vec);
                plain.zeroize();
                data.ok_or(SmError::Malformed("bad padding"))?
            }
        };
        Ok(Response::new(data, sw))
    }
}

impl<T: CardTransport> CardTransport for SecureChannel<T> {
    fn transmit(&mut self, cmd: &Command) -> Result<Response, TransportError> {
        if self.closed {
            return Err(SmError::Closed.into());
        }
        let protected = self.wrap(cmd)?;
        let result = self
            .inner
            .transmit(&protected)
            .and_then(|rsp| Ok(self.unwrap(&rsp)?));
        if result.is_err() {
            self.closed = true;
        }
        result
    }

    fn atr(&self) -> &[u8] {
        self.inner.atr()
    }
}

#[cfg(test)]
mod tests;

//! Tests against a simulated card that implements the card side of SM.

use super::*;

const K_ENC: [u8; 16] = [0x11; 16];
const K_MAC: [u8; 16] = [0x22; 16];

type Handler = Box<dyn FnMut(Command) -> Response>;

/// Card side of secure messaging, written from the spec independently of `wrap`/`unwrap`.
struct SmCard {
    ssc: [u8; 16],
    handler: Handler,
    /// Raw protected commands seen on the wire.
    wire: Vec<Vec<u8>>,
    tamper_response_mac: bool,
}

fn inc(ssc: &mut [u8; 16]) {
    let v = u128::from_be_bytes(*ssc).wrapping_add(1);
    *ssc = v.to_be_bytes();
}

impl SmCard {
    fn new(handler: impl FnMut(Command) -> Response + 'static) -> Self {
        Self {
            ssc: [0; 16],
            handler: Box::new(handler),
            wire: Vec::new(),
            tamper_response_mac: false,
        }
    }
}

impl CardTransport for SmCard {
    fn transmit(&mut self, cmd: &Command) -> Result<Response, TransportError> {
        self.wire.push(cmd.to_bytes()?);
        assert_eq!(cmd.cla & 0x0C, 0x0C, "CLA must signal SM");
        inc(&mut self.ssc);

        let mut cryptogram = None;
        let mut le = None;
        let mut mac = None;
        let mut covered = 0;
        let mut pos = 0;
        for obj in tlv::TlvIter::new(&cmd.data) {
            let obj = obj.unwrap();
            match obj.tag {
                0x87 => cryptogram = Some(obj.value.to_vec()),
                0x97 => le = Some(obj.value.to_vec()),
                0x8E => {
                    mac = Some(obj.value.to_vec());
                    covered = pos;
                }
                t => panic!("unexpected DO {t:02X}"),
            }
            pos += tlv::encode(obj.tag, obj.value).len();
        }
        let header = [cmd.cla, cmd.ins, cmd.p1, cmd.p2];
        let mut input = self.ssc.to_vec();
        if covered == 0 {
            input.extend_from_slice(&header);
        } else {
            input.extend_from_slice(&pad(&header));
            input.extend_from_slice(&cmd.data[..covered]);
        }
        if mac.as_deref() != Some(&cmac8(&K_MAC, &pad(&input))[..]) {
            return Ok(Response::new(vec![], StatusWord(0x6988)));
        }

        let mut plain = Command::new(cmd.cla & !0x0C, cmd.ins, cmd.p1, cmd.p2);
        if let Some(c) = cryptogram {
            assert_eq!(c[0], 0x01);
            let iv = iv_from_ssc(&K_ENC, &self.ssc);
            plain = plain.with_data(unpad(&cbc_decrypt(&K_ENC, &iv, &c[1..])).unwrap().to_vec());
        }
        if let Some(le) = le {
            let ne = match le.as_slice() {
                [0] => 256,
                [n] => usize::from(*n),
                [0, 0] => 65536,
                [a, b] => usize::from(u16::from_be_bytes([*a, *b])),
                _ => panic!("bad DO97"),
            };
            plain = plain.with_ne(ne);
        }

        let answer = (self.handler)(plain);
        inc(&mut self.ssc);
        let mut data = Vec::new();
        if !answer.data.is_empty() {
            let iv = iv_from_ssc(&K_ENC, &self.ssc);
            let mut v = vec![0x01];
            v.extend(cbc_encrypt(&K_ENC, &iv, &pad(&answer.data)));
            tlv::write(&mut data, 0x87, &v);
        }
        tlv::write(&mut data, 0x99, &answer.sw.0.to_be_bytes());
        let mut input = self.ssc.to_vec();
        input.extend_from_slice(&data);
        let mut mac = cmac8(&K_MAC, &pad(&input));
        if self.tamper_response_mac {
            mac[0] ^= 1;
        }
        tlv::write(&mut data, 0x8E, &mac);
        Ok(Response::new(data, StatusWord::OK))
    }

    fn atr(&self) -> &[u8] {
        &[0x3B, 0x80, 0x80, 0x01, 0x01]
    }
}

fn file() -> Vec<u8> {
    (0..300u16).map(|i| i as u8).collect()
}

fn reader_card() -> SmCard {
    SmCard::new(|cmd| match (cmd.ins, cmd.data.as_slice()) {
        (0xA4, [0xD2, 0x76, 0x00, 0x00, 0x01, 0x02]) => Response::new(vec![], StatusWord::OK),
        (0xB0, []) => {
            let off = usize::from(cmd.p1 & 0x7F) << 8 | usize::from(cmd.p2);
            let off = if cmd.p1 & 0x80 != 0 {
                usize::from(cmd.p2)
            } else {
                off
            };
            let f = file();
            let end = (off + cmd.ne.unwrap_or(0)).min(f.len());
            let sw = if end - off < cmd.ne.unwrap_or(0) {
                StatusWord::END_OF_FILE
            } else {
                StatusWord::OK
            };
            Response::new(f[off..end].to_vec(), sw)
        }
        _ => Response::new(vec![], StatusWord(0x6D00)),
    })
}

#[test]
fn select_is_case4_short() {
    let mut ch = SecureChannel::new(reader_card(), &K_ENC, &K_MAC);
    let select =
        Command::new(0x00, 0xA4, 0x04, 0x0C).with_data([0xD2, 0x76, 0x00, 0x00, 0x01, 0x02]);
    let rsp = ch.transmit(&select).unwrap();
    assert!(rsp.sw.is_ok() && rsp.data.is_empty());

    let wire = &ch.inner.wire[0];
    // 0C A4 04 0C Lc=1D [87 11 01 <16>] [8E 08 <8>] Le=00
    assert_eq!(&wire[..5], &[0x0C, 0xA4, 0x04, 0x0C, 0x1D]);
    assert_eq!(&wire[5..8], &[0x87, 0x11, 0x01]);
    assert_eq!(&wire[24..26], &[0x8E, 0x08]);
    assert_eq!(wire.len(), 5 + 0x1D + 1);
    assert_eq!(wire.last(), Some(&0x00));
}

#[test]
fn read_binary_is_case4_extended() {
    let mut ch = SecureChannel::new(reader_card(), &K_ENC, &K_MAC);
    let rsp = ch
        .transmit(&Command::new(0x00, 0xB0, 0x8C, 0x00).with_ne(25))
        .unwrap();
    assert_eq!(rsp.data, file()[..25]);
    assert!(rsp.sw.is_ok());

    // 0C B0 8C 00 00 00 0D [97 01 19] [8E 08 <8>] 00 00
    let wire = &ch.inner.wire[0];
    assert_eq!(
        &wire[..10],
        &[0x0C, 0xB0, 0x8C, 0x00, 0x00, 0x00, 0x0D, 0x97, 0x01, 0x19]
    );
    assert_eq!(&wire[wire.len() - 2..], &[0x00, 0x00]);
    assert_eq!(wire.len(), 7 + 0x0D + 2);
}

#[test]
fn ssc_stays_in_sync_over_many_commands() {
    let mut ch = SecureChannel::new(reader_card(), &K_ENC, &K_MAC);
    for off in [0u8, 0x10, 0x80] {
        let rsp = ch
            .transmit(&Command::new(0x00, 0xB0, 0x81, off).with_ne(256))
            .unwrap();
        assert_eq!(rsp.data[0], off);
    }
    // 256 bytes Ne -> DO97 01 00.
    assert_eq!(&ch.inner.wire[0][7..10], &[0x97, 0x01, 0x00]);
    // End of file is reported through DO99.
    let rsp = ch
        .transmit(&Command::new(0x00, 0xB0, 0x01, 0x00).with_ne(256))
        .unwrap();
    assert_eq!(rsp.sw, StatusWord::END_OF_FILE);
    assert_eq!(rsp.data.len(), 300 - 256);
    assert_eq!(u128::from_be_bytes(ch.state.ssc), 8);
}

#[test]
fn case1_header_only_mac() {
    let mut ch = SecureChannel::new(reader_card(), &K_ENC, &K_MAC);
    let rsp = ch.transmit(&Command::new(0x00, 0xA4, 0x04, 0x0C)).unwrap();
    // Handler answers 6D00 for SELECT without AID; what matters is that the MAC verified.
    assert_eq!(rsp.sw, StatusWord(0x6D00));
    assert_eq!(
        &ch.inner.wire[0][..7],
        &[0x0C, 0xA4, 0x04, 0x0C, 0x0A, 0x8E, 0x08]
    );
}

#[test]
fn wrong_response_mac_closes_channel() {
    let mut card = reader_card();
    card.tamper_response_mac = true;
    let mut ch = SecureChannel::new(card, &K_ENC, &K_MAC);
    let err = ch
        .transmit(&Command::new(0x00, 0xB0, 0x8C, 0x00).with_ne(25))
        .unwrap_err();
    assert!(err.to_string().contains("MAC"), "{err}");
    assert!(matches!(
        ch.transmit(&Command::new(0x00, 0xB0, 0x8C, 0x00).with_ne(25)),
        Err(TransportError::Protocol(m)) if m.contains("closed")
    ));
}

#[test]
fn plain_error_response_is_rejected() {
    // Wrong keys: the card cannot verify our MAC and answers 69 88 in plain.
    let mut ch = SecureChannel::new(reader_card(), &K_ENC, &[0x33; 16]);
    let err = ch
        .transmit(&Command::new(0x00, 0xB0, 0x8C, 0x00).with_ne(25))
        .unwrap_err();
    assert!(err.to_string().contains("69 88"), "{err}");
}

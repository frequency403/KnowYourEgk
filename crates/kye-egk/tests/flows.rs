//! Scripted end-to-end flows and error paths of `session::read_vsd`.

#![allow(clippy::unwrap_used)]

use std::io::Write;

use kye_egk::session::{Step, read_vsd};
use kye_egk::{EgkError, Vsd};
use kye_pace::{PaceError, Password};
use kye_transport::TransportError;
use kye_transport::mock::MockTransport;

const CONTACT_ATR: [u8; 11] = [
    0x3B, 0xD3, 0x96, 0xFF, 0x81, 0xB1, 0xFE, 0x45, 0x1F, 0x07, 0x80,
];
const CL_ATR: [u8; 5] = [0x3B, 0x80, 0x80, 0x01, 0x01];
const OK: [u8; 2] = [0x90, 0x00];

const SELECT_HCA: [u8; 11] = [
    0x00, 0xA4, 0x04, 0x0C, 0x06, 0xD2, 0x76, 0x00, 0x00, 0x01, 0x02,
];
const READ_STATUS: [u8; 5] = [0x00, 0xB0, 0x8C, 0x00, 0x19];
const READ_PD: [u8; 5] = [0x00, 0xB0, 0x81, 0x00, 0x00];
const READ_VD: [u8; 5] = [0x00, 0xB0, 0x82, 0x00, 0x00];

fn gzip(xml: &str) -> Vec<u8> {
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(xml.as_bytes()).unwrap();
    enc.finish().unwrap()
}

fn status_vd() -> Vec<u8> {
    let mut v = b"020240101000000".to_vec();
    v.extend([0x00, 0x50, 0x02, 0x00, 0x00, 0x00, 0x30, 0x00, 0x00, 0x04]);
    v
}

fn ef_pd() -> Vec<u8> {
    let gz = gzip(
        "<UC_PersoenlicheVersichertendatenXML><Versicherter><Versicherten_ID>X000000001\
         </Versicherten_ID><Person><Vorname>Erika</Vorname></Person></Versicherter>\
         </UC_PersoenlicheVersichertendatenXML>",
    );
    let mut ef = (gz.len() as u16).to_be_bytes().to_vec();
    ef.extend(gz);
    ef
}

fn ef_vd() -> Vec<u8> {
    let gz = gzip(
        "<UC_AllgemeineVersicherungsdatenXML><Versicherter><Versicherungsschutz>\
         <Beginn>20200101</Beginn></Versicherungsschutz></Versicherter>\
         </UC_AllgemeineVersicherungsdatenXML>",
    );
    let mut ef = vec![0x00, 0x08];
    ef.extend(((8 + gz.len() - 1) as u16).to_be_bytes());
    ef.extend([0xFF; 4]);
    ef.extend(gz);
    ef
}

fn with_sw(mut data: Vec<u8>, sw: [u8; 2]) -> Vec<u8> {
    data.extend(sw);
    data
}

fn no_progress() -> impl FnMut(Step) {
    |_| {}
}

#[test]
fn contact_plain_read() {
    let mut card = MockTransport::new(CONTACT_ATR)
        .expect(SELECT_HCA, OK)
        .expect(READ_STATUS, with_sw(status_vd(), OK))
        .expect(READ_PD, with_sw(ef_pd(), [0x62, 0x82]))
        .expect(READ_VD, with_sw(ef_vd(), [0x62, 0x82]));
    let mut steps = Vec::new();
    let r = read_vsd(&mut card, None, &mut |s| steps.push(s)).unwrap();
    assert!(card.is_done());
    assert!(!r.secured);
    assert_eq!(steps, [Step::Reading]);
    let vsd = Vsd::parse(&r.raw).unwrap();
    assert_eq!(vsd.personal.first_name.as_deref(), Some("Erika"));
    assert_eq!(vsd.insurance.coverage_start.as_deref(), Some("20200101"));
}

#[test]
fn contact_69_82_without_can_needs_can() {
    let mut card = MockTransport::new(CONTACT_ATR)
        .expect(SELECT_HCA, OK)
        .expect(READ_STATUS, [0x69, 0x82]);
    let err = read_vsd(&mut card, None, &mut no_progress()).unwrap_err();
    assert!(matches!(err, EgkError::CanRequired), "{err:?}");
}

#[test]
fn contactless_without_can_sends_nothing() {
    let mut card = MockTransport::new(CL_ATR);
    let err = read_vsd(&mut card, None, &mut no_progress()).unwrap_err();
    assert!(matches!(err, EgkError::CanRequired), "{err:?}");
}

#[test]
fn card_removed_mid_read() {
    let mut card = MockTransport::new(CONTACT_ATR)
        .expect(SELECT_HCA, OK)
        .expect(READ_STATUS, with_sw(status_vd(), OK))
        .expect_removed(READ_PD);
    let err = read_vsd(&mut card, None, &mut no_progress()).unwrap_err();
    assert!(
        matches!(err, EgkError::Transport(TransportError::CardRemoved)),
        "{err:?}"
    );
}

#[test]
fn file_shorter_than_header() {
    let mut pd = ef_pd();
    pd.truncate(pd.len() - 4);
    let mut card = MockTransport::new(CONTACT_ATR)
        .expect(SELECT_HCA, OK)
        .expect(READ_STATUS, with_sw(status_vd(), OK))
        .expect(READ_PD, with_sw(pd, [0x62, 0x82]));
    let err = read_vsd(&mut card, None, &mut no_progress()).unwrap_err();
    assert!(matches!(err, EgkError::ShortFile { .. }), "{err:?}");
}

// Points from the ICAO 9303-11 worked example; any valid curve points work here.
const PK1_PICC: &str = "04824FBA91C9CBE26BEF53A0EBE7342A3BF178CEA9F45DE0B70AA601651FBA3F57\
                        30D8C879AAA9C9F73991E61B58F4D52EB87A0A0C709A49DC63719363CCD13C54";
const PK2_PICC: &str = "049E880F842905B8B3181F7AF7CAA9F0EFB743847F44A306D2D28C1D9EC65DF6DB\
                        7764B22277A2EDDC3C265A9F018F9CB852E111B768B326904B59A0193776F094";
const CARD_ACCESS: [u8; 22] = [
    0x31, 0x14, 0x30, 0x12, 0x06, 0x0A, 0x04, 0x00, 0x7F, 0x00, 0x07, 0x02, 0x02, 0x04, 0x02, 0x02,
    0x02, 0x01, 0x02, 0x02, 0x01, 0x0D,
];

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

/// Card that runs PACE up to step 4 and then rejects T_PCD with `sw`.
fn pace_card_rejecting(sw: [u8; 2]) -> MockTransport {
    MockTransport::new(CL_ATR)
        .expect([0x00, 0xA4, 0x04, 0x0C], OK)
        .expect(
            [0x00, 0xB0, 0x9C, 0x00, 0x00],
            with_sw(CARD_ACCESS.to_vec(), OK),
        )
        .expect_prefix([0x00, 0x22, 0xC1, 0xA4], OK)
        .expect_prefix(
            [0x10, 0x86, 0x00, 0x00, 0x02, 0x7C, 0x00],
            with_sw([vec![0x7C, 0x12, 0x80, 0x10], vec![0x5A; 16]].concat(), OK),
        )
        .expect_prefix(
            [0x10, 0x86, 0x00, 0x00, 0x45, 0x7C, 0x43, 0x81, 0x41],
            with_sw([vec![0x7C, 0x43, 0x82, 0x41], hex(PK1_PICC)].concat(), OK),
        )
        .expect_prefix(
            [0x10, 0x86, 0x00, 0x00, 0x45, 0x7C, 0x43, 0x83, 0x41],
            with_sw([vec![0x7C, 0x43, 0x84, 0x41], hex(PK2_PICC)].concat(), OK),
        )
        .expect_prefix([0x00, 0x86, 0x00, 0x00, 0x0C, 0x7C, 0x0A, 0x85, 0x08], sw)
}

#[test]
fn wrong_can_is_reported() {
    let can = Password::can("000000").unwrap();
    let mut card = pace_card_rejecting([0x63, 0x00]);
    let mut steps = Vec::new();
    let err = read_vsd(&mut card, Some(&can), &mut |s| steps.push(s)).unwrap_err();
    assert!(card.is_done());
    assert_eq!(steps, [Step::Pace]);
    assert!(
        matches!(
            err,
            EgkError::Pace(PaceError::WrongPassword { retries: None })
        ),
        "{err:?}"
    );
}

#[test]
fn retry_counter_is_reported() {
    let can = Password::can("000000").unwrap();
    let mut card = pace_card_rejecting([0x63, 0xC2]);
    let err = read_vsd(&mut card, Some(&can), &mut no_progress()).unwrap_err();
    assert!(
        matches!(
            err,
            EgkError::Pace(PaceError::WrongPassword { retries: Some(2) })
        ),
        "{err:?}"
    );
}

#[test]
fn chip_token_mismatch_is_rejected() {
    // Card accepts T_PCD but answers with a token we cannot verify.
    let can = Password::can("000000").unwrap();
    let mut card = MockTransport::new(CL_ATR)
        .expect([0x00, 0xA4, 0x04, 0x0C], OK)
        .expect(
            [0x00, 0xB0, 0x9C, 0x00, 0x00],
            with_sw(CARD_ACCESS.to_vec(), OK),
        )
        .expect_prefix([0x00, 0x22, 0xC1, 0xA4], OK)
        .expect_prefix(
            [0x10, 0x86, 0x00, 0x00, 0x02],
            with_sw([vec![0x7C, 0x12, 0x80, 0x10], vec![0x5A; 16]].concat(), OK),
        )
        .expect_prefix(
            [0x10, 0x86, 0x00, 0x00, 0x45, 0x7C, 0x43, 0x81],
            with_sw([vec![0x7C, 0x43, 0x82, 0x41], hex(PK1_PICC)].concat(), OK),
        )
        .expect_prefix(
            [0x10, 0x86, 0x00, 0x00, 0x45, 0x7C, 0x43, 0x83],
            with_sw([vec![0x7C, 0x43, 0x84, 0x41], hex(PK2_PICC)].concat(), OK),
        )
        .expect_prefix(
            [0x00, 0x86, 0x00, 0x00, 0x0C],
            with_sw(vec![0x7C, 0x0A, 0x86, 0x08, 1, 2, 3, 4, 5, 6, 7, 8], OK),
        );
    let err = read_vsd(&mut card, Some(&can), &mut no_progress()).unwrap_err();
    assert!(
        matches!(err, EgkError::Pace(PaceError::TokenMismatch)),
        "{err:?}"
    );
}

#[test]
fn invalid_chip_point_is_rejected() {
    let can = Password::can("000000").unwrap();
    let mut bad = hex(PK1_PICC);
    bad[64] ^= 0x01; // no longer on the curve
    let mut card = MockTransport::new(CL_ATR)
        .expect([0x00, 0xA4, 0x04, 0x0C], OK)
        .expect(
            [0x00, 0xB0, 0x9C, 0x00, 0x00],
            with_sw(CARD_ACCESS.to_vec(), OK),
        )
        .expect_prefix([0x00, 0x22, 0xC1, 0xA4], OK)
        .expect_prefix(
            [0x10, 0x86, 0x00, 0x00, 0x02],
            with_sw([vec![0x7C, 0x12, 0x80, 0x10], vec![0x5A; 16]].concat(), OK),
        )
        .expect_prefix(
            [0x10, 0x86, 0x00, 0x00, 0x45],
            with_sw([vec![0x7C, 0x43, 0x82, 0x41], bad].concat(), OK),
        );
    let err = read_vsd(&mut card, Some(&can), &mut no_progress()).unwrap_err();
    assert!(
        matches!(err, EgkError::Pace(PaceError::InvalidPoint)),
        "{err:?}"
    );
}

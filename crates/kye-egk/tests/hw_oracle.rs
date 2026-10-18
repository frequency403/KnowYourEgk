//! Hardware oracle (manual, needs a real eGK and `KYE_CAN`):
//! EF.StatusVD, EF.PD and EF.VD must be byte-identical when read over the
//! contact interface (plain) and the contactless interface (PACE + SM).
//!
//! ```text
//! KYE_CAN=<can> cargo test -p kye-egk --test hw_oracle -- --ignored --nocapture
//! ```
//!
//! Start with the card in either reader; the test asks you to move it.
//! Card data stays in memory and is never printed, only lengths.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::{Duration, Instant};

use kye_egk::contactless::open_secure_channel;
use kye_egk::{RawVsd, read_raw_vsd};
use kye_pace::Password;
use kye_transport::pcsc::PcscProvider;
use kye_transport::{CardInterface, ReaderProvider};

const WAIT: Duration = Duration::from_secs(120);
const CHUNK: usize = 256;

fn wait_for(provider: &PcscProvider, want: CardInterface) -> String {
    println!(">> Please put the card on/in the {want:?} reader ...");
    let deadline = Instant::now() + WAIT;
    loop {
        let readers = provider.readers().expect("list readers");
        if let Some(r) = readers.iter().find(|r| r.interface() == Some(want)) {
            return r.name.clone();
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for a {want:?} card"
        );
        std::thread::sleep(Duration::from_millis(300));
    }
}

fn read_contact(provider: &PcscProvider, reader: &str) -> RawVsd {
    let mut card = provider.connect(reader).expect("connect contact");
    let mut tx = card.transaction().expect("begin transaction");
    let raw = read_raw_vsd(&mut tx, CHUNK).expect("contact read");
    tx.end(true).expect("end transaction");
    raw
}

fn read_contactless(provider: &PcscProvider, reader: &str, can: &Password) -> RawVsd {
    let mut card = provider.connect(reader).expect("connect contactless");
    let mut tx = card.transaction().expect("begin transaction");
    let raw = {
        let mut sm = open_secure_channel(&mut tx, can).expect("PACE");
        read_raw_vsd(&mut sm, CHUNK).expect("contactless read")
    };
    tx.end(true).expect("end transaction");
    raw
}

fn first_difference(a: &[u8], b: &[u8]) -> Option<usize> {
    a.iter()
        .zip(b)
        .position(|(x, y)| x != y)
        .or((a.len() != b.len()).then(|| a.len().min(b.len())))
}

#[test]
#[ignore = "needs a real eGK, two reader interfaces and KYE_CAN"]
fn contact_and_contactless_reads_are_identical() {
    let can = {
        let raw = zeroize::Zeroizing::new(std::env::var("KYE_CAN").expect("set KYE_CAN"));
        Password::can(&raw).expect("CAN must be six digits")
    };
    let provider = PcscProvider::new().expect("PC/SC context");

    // Read whichever interface currently has the card first.
    let start_cl = provider
        .readers()
        .expect("list readers")
        .iter()
        .any(|r| r.interface() == Some(CardInterface::Contactless));

    let mut contact = None;
    let mut contactless = None;
    for step in 0..2 {
        if (step == 0) == start_cl {
            let reader = wait_for(&provider, CardInterface::Contactless);
            let raw = read_contactless(&provider, &reader, &can);
            println!("contactless: {raw:?}");
            contactless = Some(raw);
        } else {
            let reader = wait_for(&provider, CardInterface::Contact);
            let raw = read_contact(&provider, &reader);
            println!("contact:     {raw:?}");
            contact = Some(raw);
        }
    }
    let (contact, contactless) = (contact.unwrap(), contactless.unwrap());

    for (name, a, b) in [
        ("EF.StatusVD", &contact.status_vd, &contactless.status_vd),
        ("EF.PD", &contact.pd, &contactless.pd),
        ("EF.VD", &contact.vd, &contactless.vd),
    ] {
        match first_difference(a, b) {
            None => println!("{name}: {} bytes identical", a.len()),
            Some(at) => panic!(
                "{name} differs at offset {at} (contact {} bytes, contactless {} bytes)",
                a.len(),
                b.len()
            ),
        }
    }
}

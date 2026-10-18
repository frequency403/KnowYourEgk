//! Hardware check of the PD/VD parser (manual, needs a real eGK):
//! prints only element paths, text lengths and which typed fields were
//! found - never card content.
//!
//! ```text
//! [KYE_CAN=<can>] cargo test -p kye-egk --test hw_structure -- --ignored --nocapture
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::Read;

use kye_egk::contactless::open_secure_channel;
use kye_egk::vsd::VdOffsets;
use kye_egk::xml::Element;
use kye_egk::{RawVsd, Vsd, read_raw_vsd};
use kye_pace::Password;
use kye_transport::pcsc::PcscProvider;
use kye_transport::{CardInterface, ReaderProvider};

fn read_any(provider: &PcscProvider) -> RawVsd {
    let readers = provider.readers().unwrap();
    let reader = readers
        .iter()
        .find(|r| r.atr.is_some())
        .expect("no card present");
    let mut card = provider.connect(&reader.name).unwrap();
    let mut tx = card.transaction().unwrap();
    let raw = if reader.interface() == Some(CardInterface::Contactless) {
        let can = zeroize::Zeroizing::new(std::env::var("KYE_CAN").expect("KYE_CAN"));
        let mut sm = open_secure_channel(&mut tx, &Password::can(&can).unwrap()).unwrap();
        read_raw_vsd(&mut sm, 256).unwrap()
    } else {
        read_raw_vsd(&mut tx, 256).unwrap()
    };
    tx.end(true).unwrap();
    raw
}

/// Bytes left after the gzip member in `data`.
fn gzip_trailing(data: &[u8]) -> usize {
    let mut d = flate2::bufread::GzDecoder::new(data);
    let mut out = Vec::new();
    d.read_to_end(&mut out).expect("gzip");
    d.into_inner().len()
}

fn print_structure(e: &Element, depth: usize) {
    let attrs: Vec<&str> = e.attributes.iter().map(|(k, _)| k.as_str()).collect();
    println!(
        "{:indent$}{}{}{}",
        "",
        e.name,
        if attrs.is_empty() {
            String::new()
        } else {
            format!(" @{attrs:?}")
        },
        if e.children.is_empty() {
            format!(" ({} chars)", e.text.chars().count())
        } else {
            String::new()
        },
        indent = depth * 2
    );
    for c in &e.children {
        print_structure(c, depth + 1);
    }
}

#[test]
#[ignore = "needs a real eGK"]
fn parser_matches_card_structure() {
    let provider = PcscProvider::new().unwrap();
    let raw = read_any(&provider);

    let off = VdOffsets::parse(&raw.vd).unwrap();
    println!(
        "EF.VD offsets: start {} end {} -> read {} bytes",
        off.start_vd,
        off.end_vd,
        off.total_len()
    );
    println!(
        "EF.VD bytes after gzip member (0 = offEndVD inclusive): {}",
        gzip_trailing(&raw.vd[usize::from(off.start_vd)..])
    );
    println!(
        "EF.PD bytes after gzip member: {}",
        gzip_trailing(&raw.pd[2..])
    );

    let vsd = Vsd::parse(&raw).expect("parse VSD");
    println!("\nEF.PD structure:");
    print_structure(&vsd.personal.xml, 1);
    println!("\nEF.VD structure:");
    print_structure(&vsd.insurance.xml, 1);

    let p = &vsd.personal;
    let i = &vsd.insurance;
    let found = |o: &Option<String>| if o.is_some() { "found" } else { "-" };
    println!("\nTyped PD fields:");
    for (n, v) in [
        ("schema_version", &p.schema_version),
        ("insurant_id", &p.insurant_id),
        ("birth_date", &p.birth_date),
        ("first_name", &p.first_name),
        ("last_name", &p.last_name),
        ("sex", &p.sex),
        ("name_prefix", &p.name_prefix),
        ("name_addition", &p.name_addition),
        ("title", &p.title),
    ] {
        println!("  {n:16} {}", found(v));
    }
    println!(
        "  street_address   {}",
        if p.street_address.is_some() {
            "found"
        } else {
            "-"
        }
    );
    println!(
        "  postbox_address  {}",
        if p.postbox_address.is_some() {
            "found"
        } else {
            "-"
        }
    );
    println!("Typed VD fields:");
    for (n, v) in [
        ("schema_version", &i.schema_version),
        ("coverage_start", &i.coverage_start),
        ("coverage_end", &i.coverage_end),
        ("payer_id", &i.payer_id),
        ("payer_name", &i.payer_name),
        ("payer_country", &i.payer_country),
        ("billing_payer_id", &i.billing_payer_id),
        ("billing_payer_name", &i.billing_payer_name),
        ("insurant_type", &i.insurant_type),
        ("special_group", &i.special_group),
        ("dmp", &i.dmp),
        ("wop", &i.wop),
    ] {
        println!("  {n:18} {}", found(v));
    }
}

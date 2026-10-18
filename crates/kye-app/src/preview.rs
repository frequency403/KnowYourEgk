//! Offscreen renders of every screen with synthetic data, for design review.
//!
//! ```text
//! cargo test -p kye-app preview -- --ignored
//! ```
//!
//! Writes PNGs to `target/ui-preview/` (or `$KYE_PREVIEW_DIR`). Needs a GPU or
//! a software Vulkan/GL driver, so it is not part of the normal test run.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::mpsc;

use eframe::egui::{self, Theme};
use egui_kittest::Harness;
use kye_egk::Vsd;
use kye_egk::model::{InsuranceData, PersonalData, StreetAddress};
use kye_egk::status_vd::{BcdVersion, StatusVd, Timestamp};
use kye_egk::xml::Element;
use kye_transport::{CardInterface, ReaderInfo};

use super::{App, Phase, Tab};
use crate::i18n::{I18n, Lang};
use crate::theme::{self, ThemeChoice};
use crate::worker::{Progress, ReadError, ReadOutcome, Worker};

const CONTACT_ATR: [u8; 11] = [
    0x3B, 0xD3, 0x96, 0xFF, 0x81, 0xB1, 0xFE, 0x45, 0x1F, 0x07, 0x80,
];
const CL_ATR: [u8; 5] = [0x3B, 0x80, 0x80, 0x01, 0x01];
const CONTACT_SLOT: &str = "HID OMNIKEY 5121 00 00";
const CL_SLOT: &str = "HID OMNIKEY 5121 00 01";

fn leaf(name: &str, text: &str) -> Element {
    Element {
        name: name.into(),
        text: text.into(),
        ..Element::default()
    }
}

fn node(name: &str, children: Vec<Element>) -> Element {
    Element {
        name: name.into(),
        children,
        ..Element::default()
    }
}

/// Made-up data in the shape of a real card.
fn vsd(incomplete: bool) -> Vsd {
    let pd_xml = node(
        "UC_PersoenlicheVersichertendatenXML",
        vec![node(
            "Versicherter",
            vec![
                leaf("Versicherten_ID", "X110000000"),
                node(
                    "Person",
                    vec![
                        leaf("Geburtsdatum", "19640812"),
                        leaf("Vorname", "Erika"),
                        leaf("Nachname", "Mustermann"),
                        leaf("Geschlecht", "W"),
                        node(
                            "StrassenAdresse",
                            vec![
                                leaf("Postleitzahl", "10117"),
                                leaf("Ort", "Berlin"),
                                node("Land", vec![leaf("Wohnsitzlaendercode", "D")]),
                                leaf("Strasse", "Musterstraße"),
                                leaf("Hausnummer", "1"),
                            ],
                        ),
                    ],
                ),
            ],
        )],
    );
    let vd_xml = node(
        "UC_AllgemeineVersicherungsdatenXML",
        vec![node(
            "Versicherter",
            vec![
                node(
                    "Versicherungsschutz",
                    vec![
                        leaf("Beginn", "20200101"),
                        node(
                            "Kostentraeger",
                            vec![
                                leaf("Kostentraegerkennung", "100000000"),
                                leaf("Kostentraegerlaendercode", "D"),
                                leaf("Name", "Example Health Insurance"),
                            ],
                        ),
                    ],
                ),
                node(
                    "Zusatzinfos",
                    vec![node(
                        "ZusatzinfosGKV",
                        vec![
                            leaf("Versichertenart", "1"),
                            node("Zusatzinfos_Abrechnung_GKV", vec![leaf("WOP", "72")]),
                        ],
                    )],
                ),
            ],
        )],
    );
    let mut personal = PersonalData::from_xml(pd_xml);
    personal.street_address = Some(StreetAddress {
        street: Some("Musterstraße".into()),
        house_number: Some("1".into()),
        addition: None,
        postal_code: Some("10117".into()),
        city: Some("Berlin".into()),
        country: Some("D".into()),
    });
    let mut insurance = InsuranceData::from_xml(vd_xml);
    insurance.schema_version = Some("5.2.0".into());
    insurance.billing_payer_name = Some("Example Billing Office".into());
    insurance.billing_payer_id = Some("100000001".into());
    Vsd {
        status: StatusVd {
            update_incomplete: incomplete,
            timestamp: Timestamp {
                year: 2026,
                month: 10,
                day: 6,
                hour: 0,
                minute: 14,
                second: 9,
            },
            version_xml: BcdVersion([0x00, 0x50, 0x02, 0x00, 0x00]),
            version_storage: BcdVersion([0x00, 0x30, 0x00, 0x00, 0x04]),
        },
        personal,
        insurance,
    }
}

fn app(readers: Vec<ReaderInfo>, phase: Phase, tab: Tab, can: &str, lang: Lang) -> App {
    let (requests, _rx) = mpsc::channel();
    let (_tx, events) = mpsc::channel();
    let mut kinds = std::collections::HashMap::new();
    kinds.insert(CONTACT_SLOT.to_owned(), CardInterface::Contact);
    kinds.insert(CL_SLOT.to_owned(), CardInterface::Contactless);
    App {
        tr: I18n::new(lang),
        theme: ThemeChoice::System,
        worker: Worker { requests, events },
        readers,
        kinds,
        unavailable: None,
        can: can.to_owned(),
        phase,
        tab,
        footer_height: 0.0,
    }
}

fn slots(contact: bool, contactless: bool) -> Vec<ReaderInfo> {
    vec![
        ReaderInfo {
            name: CONTACT_SLOT.into(),
            atr: contact.then(|| CONTACT_ATR.to_vec()),
        },
        ReaderInfo {
            name: CL_SLOT.into(),
            atr: contactless.then(|| CL_ATR.to_vec()),
        },
    ]
}

fn done(interface: CardInterface, incomplete: bool) -> Phase {
    Phase::Done(Box::new(ReadOutcome {
        interface,
        secured: interface == CardInterface::Contactless,
        vsd: vsd(incomplete),
    }))
}

fn out_dir() -> PathBuf {
    let dir = std::env::var_os("KYE_PREVIEW_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-preview"),
        PathBuf::from,
    );
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn render(name: &str, theme: Theme, size: egui::Vec2, state: App) {
    // Fonts set via `set_fonts` only apply from the next pass on, so the pass
    // run while building the harness must not draw yet.
    let mut harness = Harness::builder()
        .with_size(size)
        .with_theme(theme)
        .wgpu()
        .build_ui_state(
            |ui, (app, ready): &mut (App, bool)| {
                if *ready {
                    app.draw(ui);
                }
            },
            (state, false),
        );
    theme::install(&harness.ctx);
    egui_extras::install_image_loaders(&harness.ctx);
    harness.state_mut().1 = true;
    // Fonts and layout settle over a few frames; spinners never stop repainting.
    harness.run_steps(6);
    let image = harness.render().expect("render");
    let path = out_dir().join(format!("{name}.png"));
    image.save(&path).unwrap();
    println!("wrote {}", path.display());
}

#[test]
#[ignore = "renders PNGs for design review; needs a GPU"]
fn preview() {
    let size = egui::vec2(1280.0, 820.0);
    let en = Lang::En;
    let scenes: Vec<(&str, Theme, App)> = vec![
        (
            "01-start",
            Theme::Light,
            app(slots(false, false), Phase::Idle, Tab::Overview, "", en),
        ),
        (
            "02-contact-card",
            Theme::Light,
            app(slots(true, false), Phase::Idle, Tab::Overview, "", en),
        ),
        (
            "03-nfc-can-partial",
            Theme::Light,
            app(slots(false, true), Phase::Idle, Tab::Overview, "498", en),
        ),
        (
            "04-nfc-can-ready",
            Theme::Light,
            app(slots(false, true), Phase::Idle, Tab::Overview, "123456", en),
        ),
        (
            "05-reading-secure",
            Theme::Light,
            app(
                slots(false, true),
                Phase::Reading {
                    secure: true,
                    progress: Progress::Pace,
                },
                Tab::Overview,
                "123456",
                en,
            ),
        ),
        (
            "06-error-wrong-can",
            Theme::Light,
            app(
                slots(false, true),
                Phase::Failed(ReadError::WrongCan),
                Tab::Overview,
                "",
                en,
            ),
        ),
        (
            "07-result-overview",
            Theme::Light,
            app(
                slots(false, true),
                done(CardInterface::Contactless, true),
                Tab::Overview,
                "",
                en,
            ),
        ),
        (
            "08-result-personal",
            Theme::Light,
            app(
                slots(true, false),
                done(CardInterface::Contact, false),
                Tab::Personal,
                "",
                en,
            ),
        ),
        (
            "09-result-raw",
            Theme::Light,
            app(
                slots(false, true),
                done(CardInterface::Contactless, false),
                Tab::Raw,
                "",
                en,
            ),
        ),
        (
            "10-dark-start",
            Theme::Dark,
            app(slots(false, true), Phase::Idle, Tab::Overview, "12", en),
        ),
        (
            "11-dark-result",
            Theme::Dark,
            app(
                slots(false, true),
                done(CardInterface::Contactless, true),
                Tab::Overview,
                "",
                en,
            ),
        ),
        (
            "12-de-result",
            Theme::Light,
            app(
                slots(true, false),
                done(CardInterface::Contact, false),
                Tab::Insurance,
                "",
                Lang::De,
            ),
        ),
    ];
    for (name, theme, state) in scenes {
        render(name, theme, size, state);
    }

    // Minimum window size: the sidebar must scroll instead of overlapping, and
    // the result header must wrap.
    let min = egui::vec2(900.0, 600.0);
    render(
        "20-min-nfc-start",
        Theme::Dark,
        min,
        app(
            slots(false, true),
            Phase::Idle,
            Tab::Overview,
            "12",
            Lang::De,
        ),
    );
    render(
        "21-min-result",
        Theme::Light,
        min,
        app(
            slots(false, true),
            done(CardInterface::Contactless, true),
            Tab::Overview,
            "",
            Lang::De,
        ),
    );
}

//! egui front end. All card I/O happens on the worker thread.

use eframe::egui::{self, Color32, RichText};
use kye_egk::model::format_date;
use kye_egk::xml::Element;
use kye_egk::{InsuranceData, PersonalData, StatusVd};
use kye_pace::Password;
use kye_transport::{CardInterface, ReaderInfo};
use zeroize::Zeroize;

use crate::worker::{self, Event, ReadError, ReadOutcome, Request, Worker};

const WARN: Color32 = Color32::from_rgb(0xD9, 0x8C, 0x00);
const ERROR: Color32 = Color32::from_rgb(0xD0, 0x40, 0x40);

pub struct App {
    worker: Worker,
    readers: Vec<ReaderInfo>,
    unavailable: Option<String>,
    /// CAN as typed by the user. Kept in memory only and zeroized on drop.
    can: String,
    busy: Option<&'static str>,
    result: Option<Result<ReadOutcome, ReadError>>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> std::io::Result<Self> {
        let ctx = cc.egui_ctx.clone();
        let worker = worker::spawn(move || ctx.request_repaint())?;
        Ok(Self {
            worker,
            readers: Vec::new(),
            unavailable: None,
            can: String::new(),
            busy: None,
            result: None,
        })
    }

    fn drain_events(&mut self) {
        while let Ok(ev) = self.worker.events.try_recv() {
            match ev {
                Event::Readers(r) => {
                    self.readers = r;
                    self.unavailable = None;
                }
                Event::Progress(p) => self.busy = Some(p),
                Event::Finished(r) => {
                    self.busy = None;
                    self.result = Some(*r);
                }
                Event::Unavailable(e) => {
                    self.unavailable = Some(e);
                    self.readers.clear();
                }
            }
        }
    }

    fn start_read(&mut self, reader: &str) {
        let can = Password::can(&self.can).ok();
        let req = Request::Read {
            reader: reader.to_owned(),
            can,
        };
        if self.worker.requests.send(req).is_ok() {
            self.busy = Some("Starte …");
            self.result = None;
        }
    }

    fn reader_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Kartenleser");
        ui.add_space(4.0);
        if let Some(e) = &self.unavailable {
            ui.colored_label(ERROR, format!("PC/SC nicht verfügbar: {e}"));
            return;
        }
        if self.readers.is_empty() {
            ui.label("Kein Kartenleser gefunden.");
        }

        let can_valid = Password::can(&self.can).is_ok();
        let mut start = None;
        for r in &self.readers {
            ui.group(|ui| {
                ui.label(RichText::new(short_name(&r.name)).strong());
                let (label, can_read) = match r.interface() {
                    None => ("leer", false),
                    Some(CardInterface::Contact) => ("Karte gesteckt (kontaktbehaftet)", true),
                    Some(CardInterface::Contactless) => ("Karte aufgelegt (kontaktlos)", can_valid),
                    Some(CardInterface::Unknown) => ("unbekannte Karte", true),
                };
                ui.label(label);
                let enabled = can_read && self.busy.is_none();
                if r.atr.is_some()
                    && ui
                        .add_enabled(enabled, egui::Button::new("Lesen"))
                        .clicked()
                {
                    start = Some(r.name.clone());
                }
                if r.interface() == Some(CardInterface::Contactless) && !can_valid {
                    ui.small("CAN eingeben, um kontaktlos zu lesen.");
                }
            });
        }
        if let Some(name) = start {
            self.start_read(&name);
        }

        ui.add_space(12.0);
        ui.label("CAN (6 Ziffern, Kartenvorderseite oben rechts):");
        ui.add(
            egui::TextEdit::singleline(&mut self.can)
                .password(true)
                .char_limit(6)
                .desired_width(120.0)
                .hint_text("······"),
        );
        ui.small("Die CAN wird nur für kontaktloses Lesen benötigt und nicht gespeichert.");
    }

    fn result_panel(&mut self, ui: &mut egui::Ui) {
        if let Some(step) = self.busy {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(step);
            });
            return;
        }
        let mut clear = false;
        match &self.result {
            None => {
                ui.heading("Willkommen");
                ui.label(
                    "Stecke die eGK in den Kartenleser oder lege sie auf und klicke auf „Lesen“.",
                );
            }
            Some(Err(e)) => {
                ui.colored_label(ERROR, e.to_string());
            }
            Some(Ok(outcome)) => {
                ui.horizontal(|ui| {
                    ui.heading("Versichertenstammdaten");
                    ui.label(match (outcome.interface, outcome.secured) {
                        (CardInterface::Contactless, _) => "· kontaktlos gelesen (PACE)",
                        (_, true) => "· kontaktbehaftet gelesen (PACE)",
                        _ => "· kontaktbehaftet gelesen",
                    });
                    if ui.button("Daten verwerfen").clicked() {
                        clear = true;
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    show_status(ui, &outcome.vsd.status);
                    show_personal(ui, &outcome.vsd.personal);
                    show_insurance(ui, &outcome.vsd.insurance);
                    egui::CollapsingHeader::new("Alle Felder (Rohdaten)")
                        .default_open(false)
                        .show(ui, |ui| {
                            show_leaves(ui, "pd", &outcome.vsd.personal.xml);
                            show_leaves(ui, "vd", &outcome.vsd.insurance.xml);
                        });
                });
            }
        }
        if clear {
            self.result = None;
        }
    }
}

impl eframe::App for App {
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_events();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::left("readers")
            .resizable(true)
            .default_size(300.0)
            .show(ui, |ui| self.reader_panel(ui));
        egui::CentralPanel::default_margins().show(ui, |ui| self.result_panel(ui));
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.can.zeroize();
    }
}

/// Strips the serial number in parentheses that some drivers append.
fn short_name(name: &str) -> String {
    let mut out = String::new();
    let mut depth = 0u32;
    for c in name.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn grid(ui: &mut egui::Ui, id: &str, rows: &[(&str, Option<String>)]) {
    egui::Grid::new(id)
        .num_columns(2)
        .spacing([24.0, 6.0])
        .striped(true)
        .show(ui, |ui| {
            for (label, value) in rows {
                if let Some(v) = value {
                    ui.label(*label);
                    ui.label(RichText::new(v).strong());
                    ui.end_row();
                }
            }
        });
}

fn show_status(ui: &mut egui::Ui, s: &StatusVd) {
    if s.update_incomplete {
        ui.colored_label(
            WARN,
            "⚠ Die letzte Aktualisierung der Versichertendaten wurde nicht abgeschlossen. \
             Die Daten können unvollständig oder veraltet sein.",
        );
        ui.add_space(6.0);
    }
}

fn date(v: &Option<String>) -> Option<String> {
    v.as_deref().map(format_date)
}

fn sex(v: &Option<String>) -> Option<String> {
    v.as_deref().map(|s| {
        match s {
            "M" => "männlich",
            "W" => "weiblich",
            "D" => "divers",
            "X" => "unbestimmt",
            other => other,
        }
        .to_owned()
    })
}

fn insurant_type(v: &Option<String>) -> Option<String> {
    v.as_deref().map(|s| match s {
        "1" => "Mitglied (1)".to_owned(),
        "3" => "Familienversicherter (3)".to_owned(),
        "5" => "Rentner (5)".to_owned(),
        other => other.to_owned(),
    })
}

fn join(parts: &[&Option<String>]) -> Option<String> {
    let s: Vec<&str> = parts.iter().filter_map(|p| p.as_deref()).collect();
    (!s.is_empty()).then(|| s.join(" "))
}

fn show_personal(ui: &mut egui::Ui, p: &PersonalData) {
    ui.add_space(4.0);
    ui.strong("Persönliche Daten");
    let name = join(&[
        &p.title,
        &p.first_name,
        &p.name_prefix,
        &p.last_name,
        &p.name_addition,
    ]);
    let mut rows = vec![
        ("Name", name),
        ("Geburtsdatum", date(&p.birth_date)),
        ("Geschlecht", sex(&p.sex)),
        ("Versicherten-ID", p.insurant_id.clone()),
    ];
    if let Some(a) = &p.street_address {
        rows.push(("Straße", join(&[&a.street, &a.house_number])));
        rows.push(("Zusatz", a.addition.clone()));
        rows.push(("Ort", join(&[&a.postal_code, &a.city])));
        rows.push(("Land", a.country.clone()));
    }
    if let Some(a) = &p.postbox_address {
        rows.push(("Postfach", a.postbox.clone()));
        rows.push(("Postfach-Ort", join(&[&a.postal_code, &a.city])));
    }
    grid(ui, "pd", &rows);
}

fn show_insurance(ui: &mut egui::Ui, i: &InsuranceData) {
    ui.add_space(10.0);
    ui.strong("Versicherungsdaten");
    let ende = date(&i.coverage_end).or_else(|| Some("unbefristet".to_owned()));
    grid(
        ui,
        "vd",
        &[
            ("Krankenkasse", i.payer_name.clone()),
            ("IK der Kasse", i.payer_id.clone()),
            ("Abrechnende Stelle", i.billing_payer_name.clone()),
            ("IK abrechnend", i.billing_payer_id.clone()),
            ("Versicherungsschutz ab", date(&i.coverage_start)),
            ("Versicherungsschutz bis", ende),
            ("Versichertenart", insurant_type(&i.insurant_type)),
            ("Besondere Personengruppe", i.special_group.clone()),
            ("DMP-Kennzeichnung", i.dmp.clone()),
            ("WOP", i.wop.clone()),
            ("Schema", i.schema_version.clone()),
        ],
    );
}

fn show_leaves(ui: &mut egui::Ui, id: &str, root: &Element) {
    egui::Grid::new(id)
        .num_columns(2)
        .striped(true)
        .show(ui, |ui| {
            for (path, text) in root.leaves() {
                ui.monospace(path);
                ui.label(text);
                ui.end_row();
            }
        });
}

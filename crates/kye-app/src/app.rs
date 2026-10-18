//! egui front end. All card I/O happens on the worker thread.

use eframe::egui::{self, Color32, RichText};
use kye_egk::xml::Element;
use kye_egk::{InsuranceData, PersonalData, StatusVd};
use kye_pace::Password;
use kye_transport::{CardInterface, ReaderInfo};
use zeroize::Zeroize;

use crate::i18n::{I18n, Lang};
use crate::worker::{self, Event, Progress, ReadError, ReadOutcome, Request, Worker};

const WARN: Color32 = Color32::from_rgb(0xD9, 0x8C, 0x00);
const ERROR: Color32 = Color32::from_rgb(0xD0, 0x40, 0x40);

pub struct App {
    tr: I18n,
    worker: Worker,
    readers: Vec<ReaderInfo>,
    unavailable: Option<String>,
    /// CAN as typed by the user. Kept in memory only and zeroized on drop.
    can: String,
    busy: Option<Progress>,
    result: Option<Result<ReadOutcome, ReadError>>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> std::io::Result<Self> {
        let ctx = cc.egui_ctx.clone();
        let worker = worker::spawn(move || ctx.request_repaint())?;
        Ok(Self {
            tr: I18n::new(Lang::detect()),
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
            self.busy = Some(Progress::Starting);
            self.result = None;
        }
    }

    fn language_selector(&mut self, ui: &mut egui::Ui) {
        let mut lang = self.tr.lang();
        ui.horizontal(|ui| {
            ui.label(self.tr.t("language-label"));
            egui::ComboBox::from_id_salt("language")
                .selected_text(lang.native_name())
                .show_ui(ui, |ui| {
                    for l in Lang::ALL {
                        ui.selectable_value(&mut lang, l, l.native_name());
                    }
                });
        });
        if lang != self.tr.lang() {
            self.tr = I18n::new(lang);
        }
    }

    fn reader_panel(&mut self, ui: &mut egui::Ui) {
        let tr = &self.tr;
        ui.heading(tr.t("readers-heading"));
        ui.add_space(4.0);
        if let Some(e) = &self.unavailable {
            ui.colored_label(
                ERROR,
                tr.t_with("readers-unavailable", &[("detail", e.clone().into())]),
            );
            return;
        }
        if self.readers.is_empty() {
            ui.label(tr.t("readers-none"));
        }

        let can_valid = Password::can(&self.can).is_ok();
        let mut start = None;
        for r in &self.readers {
            ui.group(|ui| {
                ui.label(RichText::new(short_name(&r.name)).strong());
                let (label, can_read) = match r.interface() {
                    None => (tr.t("slot-empty"), false),
                    Some(CardInterface::Contact) => (tr.t("slot-contact"), true),
                    Some(CardInterface::Contactless) => (tr.t("slot-contactless"), can_valid),
                    Some(CardInterface::Unknown) => (tr.t("slot-unknown"), true),
                };
                ui.label(label);
                let enabled = can_read && self.busy.is_none();
                if r.atr.is_some()
                    && ui
                        .add_enabled(enabled, egui::Button::new(tr.t("button-read")))
                        .clicked()
                {
                    start = Some(r.name.clone());
                }
                if r.interface() == Some(CardInterface::Contactless) && !can_valid {
                    ui.small(tr.t("can-needed-hint"));
                }
            });
        }
        if let Some(name) = start {
            self.start_read(&name);
        }

        ui.add_space(12.0);
        ui.label(self.tr.t("can-label"));
        ui.add(
            egui::TextEdit::singleline(&mut self.can)
                .password(true)
                .char_limit(6)
                .desired_width(120.0)
                .hint_text("······"),
        );
        ui.small(self.tr.t("can-note"));

        ui.add_space(12.0);
        ui.separator();
        self.language_selector(ui);
    }

    fn result_panel(&mut self, ui: &mut egui::Ui) {
        let tr = &self.tr;
        if let Some(step) = self.busy {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(progress_text(tr, step));
            });
            return;
        }
        let mut clear = false;
        match &self.result {
            None => {
                ui.heading(tr.t("welcome-heading"));
                ui.label(tr.t("welcome-text"));
            }
            Some(Err(e)) => {
                ui.colored_label(ERROR, error_text(tr, e));
            }
            Some(Ok(outcome)) => {
                ui.horizontal(|ui| {
                    ui.heading(tr.t("result-heading"));
                    ui.label(match (outcome.interface, outcome.secured) {
                        (CardInterface::Contactless, _) => tr.t("read-via-contactless"),
                        (_, true) => tr.t("read-via-contact-pace"),
                        _ => tr.t("read-via-contact"),
                    });
                    if ui.button(tr.t("button-discard")).clicked() {
                        clear = true;
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    show_status(ui, tr, &outcome.vsd.status);
                    show_personal(ui, tr, &outcome.vsd.personal);
                    show_insurance(ui, tr, &outcome.vsd.insurance);
                    egui::CollapsingHeader::new(tr.t("raw-fields"))
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

fn grid(ui: &mut egui::Ui, id: &str, rows: &[(String, Option<String>)]) {
    egui::Grid::new(id)
        .num_columns(2)
        .spacing([24.0, 6.0])
        .striped(true)
        .show(ui, |ui| {
            for (label, value) in rows {
                if let Some(v) = value {
                    ui.label(label);
                    ui.label(RichText::new(v).strong());
                    ui.end_row();
                }
            }
        });
}

fn progress_text(tr: &I18n, p: Progress) -> String {
    tr.t(match p {
        Progress::Starting => "progress-starting",
        Progress::Connecting => "progress-connecting",
        Progress::Reading => "progress-reading",
        Progress::Pace => "progress-pace",
        Progress::ReadingSecure => "progress-reading-secure",
        Progress::Parsing => "progress-parsing",
    })
}

fn error_text(tr: &I18n, e: &ReadError) -> String {
    match e {
        ReadError::CanRequired => tr.t("error-can-required"),
        ReadError::WrongCan => tr.t("error-wrong-can"),
        ReadError::CardRemoved => tr.t("error-card-removed"),
        ReadError::ReaderGone => tr.t("error-reader-gone"),
        ReadError::NoCard => tr.t("error-no-card"),
        ReadError::Unsupported => tr.t("error-unsupported"),
        ReadError::ServiceUnavailable => tr.t("error-service"),
        ReadError::Card(detail) => tr.t_with("error-card", &[("detail", detail.clone().into())]),
    }
}

fn show_status(ui: &mut egui::Ui, tr: &I18n, s: &StatusVd) {
    if s.update_incomplete {
        ui.colored_label(WARN, tr.t("update-incomplete"));
        ui.add_space(6.0);
    }
}

/// Formats `YYYYMMDD` with the language's `date-format`; other input is shown as is.
fn date(tr: &I18n, v: &Option<String>) -> Option<String> {
    v.as_deref().map(|d| {
        if d.len() == 8 && d.bytes().all(|b| b.is_ascii_digit()) {
            tr.t_with(
                "date-format",
                &[
                    ("year", d[0..4].to_owned().into()),
                    ("month", d[4..6].to_owned().into()),
                    ("day", d[6..8].to_owned().into()),
                ],
            )
        } else {
            d.to_owned()
        }
    })
}

fn sex(tr: &I18n, v: &Option<String>) -> Option<String> {
    v.as_deref().map(|s| match s {
        "M" => tr.t("sex-male"),
        "W" => tr.t("sex-female"),
        "D" => tr.t("sex-diverse"),
        "X" => tr.t("sex-undetermined"),
        other => other.to_owned(),
    })
}

fn insurant_type(tr: &I18n, v: &Option<String>) -> Option<String> {
    v.as_deref().map(|s| {
        let code = [("code", s.to_owned().into())];
        match s {
            "1" => tr.t_with("insurant-type-member", &code),
            "3" => tr.t_with("insurant-type-family", &code),
            "5" => tr.t_with("insurant-type-pensioner", &code),
            other => other.to_owned(),
        }
    })
}

fn join(parts: &[&Option<String>]) -> Option<String> {
    let s: Vec<&str> = parts.iter().filter_map(|p| p.as_deref()).collect();
    (!s.is_empty()).then(|| s.join(" "))
}

fn show_personal(ui: &mut egui::Ui, tr: &I18n, p: &PersonalData) {
    ui.add_space(4.0);
    ui.strong(tr.t("section-personal"));
    let name = join(&[
        &p.title,
        &p.first_name,
        &p.name_prefix,
        &p.last_name,
        &p.name_addition,
    ]);
    let mut rows = vec![
        (tr.t("field-name"), name),
        (tr.t("field-birth-date"), date(tr, &p.birth_date)),
        (tr.t("field-sex"), sex(tr, &p.sex)),
        (tr.t("field-insurant-id"), p.insurant_id.clone()),
    ];
    if let Some(a) = &p.street_address {
        rows.push((tr.t("field-street"), join(&[&a.street, &a.house_number])));
        rows.push((tr.t("field-address-addition"), a.addition.clone()));
        rows.push((tr.t("field-city"), join(&[&a.postal_code, &a.city])));
        rows.push((tr.t("field-country"), a.country.clone()));
    }
    if let Some(a) = &p.postbox_address {
        rows.push((tr.t("field-postbox"), a.postbox.clone()));
        rows.push((tr.t("field-postbox-city"), join(&[&a.postal_code, &a.city])));
    }
    grid(ui, "pd", &rows);
}

fn show_insurance(ui: &mut egui::Ui, tr: &I18n, i: &InsuranceData) {
    ui.add_space(10.0);
    ui.strong(tr.t("section-insurance"));
    let end = date(tr, &i.coverage_end).or_else(|| Some(tr.t("coverage-open-ended")));
    grid(
        ui,
        "vd",
        &[
            (tr.t("field-payer"), i.payer_name.clone()),
            (tr.t("field-payer-id"), i.payer_id.clone()),
            (tr.t("field-billing-payer"), i.billing_payer_name.clone()),
            (tr.t("field-billing-payer-id"), i.billing_payer_id.clone()),
            (tr.t("field-coverage-start"), date(tr, &i.coverage_start)),
            (tr.t("field-coverage-end"), end),
            (
                tr.t("field-insurant-type"),
                insurant_type(tr, &i.insurant_type),
            ),
            (tr.t("field-special-group"), i.special_group.clone()),
            (tr.t("field-dmp"), i.dmp.clone()),
            (tr.t("field-wop"), i.wop.clone()),
            (tr.t("field-schema"), i.schema_version.clone()),
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

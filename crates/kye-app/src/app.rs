//! egui front end, laid out after the clickable UI concept. All card I/O
//! happens on the worker thread; all colors come from `theme::Palette`.

use std::collections::HashMap;

use eframe::egui::{
    self, Align, Color32, CornerRadius, CursorIcon, Frame, Layout, Margin, RichText, ScrollArea,
    Sense, Stroke, StrokeKind, Vec2, WidgetInfo, WidgetType, vec2,
};
use kye_egk::xml::Element;
use kye_egk::{InsuranceData, PersonalData};
use kye_pace::Password;
use kye_transport::{CardInterface, ReaderInfo};
use zeroize::Zeroize;

use crate::i18n::{I18n, Lang};
use crate::icons;
use crate::theme::{self, Palette, ThemeChoice, bold, medium, mono, regular, semibold};
use crate::worker::{self, Event, Progress, ReadError, ReadOutcome, Request, Worker};

/// Project page, linked in the sidebar footer.
const REPO_URL: &str = "https://github.com/frequency403/KnowYourEgk";
/// Shown as the link text next to the GitHub mark.
const REPO_OWNER: &str = "frequency403";
/// Donation page (Buy Me a Coffee, Ko-fi, GitHub Sponsors, …). The donate link
/// is hidden while this is `None`.
const DONATE_URL: Option<&str> = None;

const GITHUB_MARK: &[u8] = include_bytes!("../assets/icons/github-mark.svg");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Overview,
    Personal,
    Insurance,
    Raw,
}

enum Phase {
    Idle,
    Reading { secure: bool, progress: Progress },
    Failed(ReadError),
    Done(Box<ReadOutcome>),
}

/// Where a button in the main panel leads.
#[derive(Clone, Copy)]
enum Action {
    EnterCanAgain,
    BackToStart,
}

pub struct App {
    tr: I18n,
    theme: ThemeChoice,
    worker: Worker,
    readers: Vec<ReaderInfo>,
    /// Interface last seen per reader slot, to label empty slots.
    kinds: HashMap<String, CardInterface>,
    unavailable: Option<String>,
    /// CAN as typed by the user. Kept in memory only, cleared after each read.
    can: String,
    phase: Phase,
    tab: Tab,
    /// Height of the sidebar footer in the last frame, to pin it to the bottom.
    footer_height: f32,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> std::io::Result<Self> {
        theme::install(&cc.egui_ctx);
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let theme = ThemeChoice::System;
        theme.apply(&cc.egui_ctx);
        let ctx = cc.egui_ctx.clone();
        let worker = worker::spawn(move || ctx.request_repaint())?;
        Ok(Self {
            tr: I18n::new(Lang::detect()),
            theme,
            worker,
            readers: Vec::new(),
            kinds: HashMap::new(),
            unavailable: None,
            can: String::new(),
            phase: Phase::Idle,
            tab: Tab::Overview,
            footer_height: 0.0,
        })
    }

    fn drain_events(&mut self) {
        while let Ok(ev) = self.worker.events.try_recv() {
            match ev {
                Event::Readers(readers) => {
                    for r in &readers {
                        if let Some(i) = r.interface() {
                            self.kinds.insert(r.name.clone(), i);
                        }
                    }
                    self.readers = readers;
                    self.unavailable = None;
                }
                Event::Progress(p) => {
                    if let Phase::Reading { secure, progress } = &mut self.phase {
                        *progress = p;
                        *secure |= p == Progress::Pace;
                    }
                }
                Event::Finished(result) => {
                    self.can.zeroize();
                    self.tab = Tab::Overview;
                    self.phase = match *result {
                        Ok(outcome) => Phase::Done(Box::new(outcome)),
                        Err(e) => Phase::Failed(e),
                    };
                }
                Event::Unavailable(e) => {
                    self.unavailable = Some(e);
                    self.readers.clear();
                }
            }
        }
    }

    fn reading(&self) -> bool {
        matches!(self.phase, Phase::Reading { .. })
    }

    fn start_read(&mut self, reader: &str, interface: CardInterface) {
        let req = Request::Read {
            reader: reader.to_owned(),
            can: Password::can(&self.can).ok(),
        };
        if self.worker.requests.send(req).is_ok() {
            self.phase = Phase::Reading {
                secure: interface == CardInterface::Contactless,
                progress: Progress::Starting,
            };
        }
    }

    // ---------------------------------------------------------------- header

    fn header(&mut self, ui: &mut egui::Ui) {
        let pal = *Palette::of(ui.ctx());
        ui.horizontal(|ui| {
            // Tall enough for the combo boxes, so everything centers on one line.
            ui.set_min_height(40.0);
            icons::paint(ui, vec2(32.0, 32.0), |p, r| icons::logo(p, r, pal.accent));
            ui.add_space(4.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.label(RichText::new("KnowYourEgk").font(bold(17.0)).color(pal.ink));
                ui.label(
                    RichText::new(self.tr.t("header-tagline"))
                        .font(regular(12.0))
                        .color(pal.muted),
                );
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                self.theme_selector(ui, &pal);
                ui.add_space(12.0);
                self.language_selector(ui, &pal);
                // The offline note goes first when the window gets narrow.
                if ui.available_width() > 420.0 {
                    ui.add_space(16.0);
                    ui.label(
                        RichText::new(self.tr.t("header-offline"))
                            .font(regular(13.0))
                            .color(pal.muted),
                    );
                    icons::paint(ui, vec2(16.0, 16.0), |p, r| icons::lock(p, r, pal.muted));
                }
            });
        });
    }

    fn language_selector(&mut self, ui: &mut egui::Ui, pal: &Palette) {
        let mut lang = self.tr.lang();
        egui::ComboBox::from_id_salt("language")
            .selected_text(lang.native_name())
            .show_ui(ui, |ui| {
                for l in Lang::ALL {
                    ui.selectable_value(&mut lang, l, l.native_name());
                }
            });
        ui.label(
            RichText::new(self.tr.t("language-label"))
                .font(regular(13.0))
                .color(pal.muted),
        );
        if lang != self.tr.lang() {
            self.tr = I18n::new(lang);
        }
    }

    fn theme_name(&self, t: ThemeChoice) -> String {
        self.tr.t(match t {
            ThemeChoice::System => "theme-system",
            ThemeChoice::Light => "theme-light",
            ThemeChoice::Dark => "theme-dark",
        })
    }

    fn theme_selector(&mut self, ui: &mut egui::Ui, pal: &Palette) {
        let mut choice = self.theme;
        egui::ComboBox::from_id_salt("theme")
            .selected_text(self.theme_name(choice))
            .show_ui(ui, |ui| {
                for t in ThemeChoice::ALL {
                    ui.selectable_value(&mut choice, t, self.theme_name(t));
                }
            });
        ui.label(
            RichText::new(self.tr.t("theme-label"))
                .font(regular(13.0))
                .color(pal.muted),
        );
        if choice != self.theme {
            self.theme = choice;
            choice.apply(ui.ctx());
        }
    }

    // --------------------------------------------------------------- sidebar

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        let pal = *Palette::of(ui.ctx());
        let viewport = ui.available_height();
        ScrollArea::vertical()
            .id_salt("sidebar")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 14.0;
                let top = ui.cursor().top();
                ui.label(
                    RichText::new(self.tr.t("readers-heading").to_uppercase())
                        .font(semibold(13.0))
                        .color(pal.muted),
                );
                if let Some(e) = &self.unavailable {
                    ui.label(
                        RichText::new(
                            self.tr
                                .t_with("readers-unavailable", &[("detail", e.clone().into())]),
                        )
                        .color(pal.error_ink),
                    );
                } else if self.readers.is_empty() {
                    ui.label(RichText::new(self.tr.t("readers-none")).color(pal.muted));
                }

                let mut start = None;
                for r in self.readers.clone() {
                    if let Some(i) = self.reader_card(ui, &pal, &r) {
                        start = Some((r.name.clone(), i));
                    }
                }
                if let Some((name, interface)) = start {
                    self.start_read(&name, interface);
                }

                // Pin the footer to the bottom while there is room; otherwise it
                // simply follows the reader cards and the sidebar scrolls.
                let used = ui.cursor().top() - top;
                let free = viewport - used - self.footer_height - 14.0;
                if free > 0.0 {
                    ui.add_space(free);
                }
                let footer = ui.scope(|ui| self.sidebar_footer(ui, &pal)).response.rect;
                self.footer_height = footer.height();
            });
    }

    fn sidebar_footer(&self, ui: &mut egui::Ui, pal: &Palette) {
        ui.spacing_mut().item_spacing.y = 10.0;
        ui.label(
            RichText::new(self.tr.t("readers-footer"))
                .font(regular(12.0))
                .color(pal.muted),
        );
        let y = ui.cursor().top();
        ui.painter()
            .hline(ui.max_rect().x_range(), y, Stroke::new(1.0, pal.line));
        ui.add_space(2.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.add(
                egui::Image::from_bytes("bytes://github-mark.svg", GITHUB_MARK)
                    .tint(pal.ink)
                    .fit_to_exact_size(vec2(16.0, 16.0)),
            );
            ui.hyperlink_to(RichText::new(REPO_OWNER).font(medium(13.0)), REPO_URL)
                .on_hover_text(self.tr.t("footer-github"));
            if let Some(url) = DONATE_URL {
                ui.add_space(14.0);
                icons::paint(ui, vec2(16.0, 16.0), |p, r| icons::cup(p, r, pal.ink));
                ui.hyperlink_to(
                    RichText::new(self.tr.t("footer-donate")).font(medium(13.0)),
                    url,
                );
            }
        });
    }

    /// Draws one reader slot; returns the card interface if "Read" was clicked.
    fn reader_card(
        &mut self,
        ui: &mut egui::Ui,
        pal: &Palette,
        r: &ReaderInfo,
    ) -> Option<CardInterface> {
        let iface = r.interface();
        let has_card = iface.is_some();
        let kind = iface.or_else(|| self.kinds.get(&r.name).copied());
        let is_nfc = kind == Some(CardInterface::Contactless);
        let reading = self.reading();
        let mut clicked = None;

        let stroke = if has_card {
            Stroke::new(2.0, pal.accent)
        } else {
            Stroke::new(1.0, pal.line)
        };
        Frame::new()
            .fill(pal.surface)
            .stroke(stroke)
            .corner_radius(CornerRadius::same(12))
            .inner_margin(Margin::same(16))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.horizontal_top(|ui| {
                    let (bg, ink) = if has_card {
                        (pal.accent_soft, pal.accent)
                    } else {
                        (pal.neutral_soft, pal.faint)
                    };
                    icons::paint(ui, vec2(40.0, 40.0), |p, rect| {
                        p.rect_filled(rect, CornerRadius::same(10), bg);
                        let inner = egui::Rect::from_center_size(rect.center(), Vec2::splat(22.0));
                        if is_nfc {
                            icons::nfc(p, inner, ink);
                        } else {
                            icons::contact(p, inner, ink);
                        }
                    });
                    ui.add_space(4.0);
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(short_name(&r.name))
                                .font(semibold(14.0))
                                .color(pal.ink),
                        );
                        ui.add_space(2.0);
                        let kind_key = match kind {
                            Some(CardInterface::Contact) => "slot-kind-contact",
                            Some(CardInterface::Contactless) => "slot-kind-contactless",
                            _ => "slot-kind-unknown",
                        };
                        ui.label(
                            RichText::new(self.tr.t(kind_key))
                                .font(regular(13.0))
                                .color(pal.muted),
                        );
                        ui.add_space(8.0);
                        let status_key = match iface {
                            None => "status-no-card",
                            Some(CardInterface::Contact) => "status-inserted",
                            Some(CardInterface::Contactless) => "status-on-reader",
                            Some(CardInterface::Unknown) => "status-unknown-card",
                        };
                        let (dot, ink) = if has_card {
                            (pal.accent, pal.accent_text)
                        } else {
                            (pal.control_border, pal.muted)
                        };
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            icons::paint(ui, vec2(8.0, 16.0), |p, rect| {
                                p.circle_filled(rect.center(), 4.0, dot);
                            });
                            ui.label(
                                RichText::new(self.tr.t(status_key))
                                    .font(medium(13.0))
                                    .color(ink),
                            );
                        });
                    });
                });

                if has_card && is_nfc {
                    ui.add_space(14.0);
                    self.can_field(ui, pal);
                }

                if has_card {
                    ui.add_space(14.0);
                    let needs_can = is_nfc && Password::can(&self.can).is_err();
                    let label = if reading {
                        self.tr.t("button-reading")
                    } else if needs_can {
                        self.tr.t("button-enter-can")
                    } else {
                        self.tr.t("button-read")
                    };
                    let enabled = !reading && !needs_can;
                    if button(ui, pal, &label, Kind::Primary, enabled, true).clicked() {
                        clicked = iface;
                    }
                }
            });
        clicked
    }

    fn can_field(&mut self, ui: &mut egui::Ui, pal: &Palette) {
        ui.spacing_mut().item_spacing.y = 8.0;
        ui.label(
            RichText::new(self.tr.t("can-label"))
                .font(semibold(13.0))
                .color(pal.ink),
        );
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            icons::paint(ui, vec2(64.0, 42.0), |p, r| icons::can_card(p, r, pal))
                .on_hover_text(self.tr.t("can-illustration"));
            ui.add(
                egui::Label::new(
                    RichText::new(self.tr.t("can-where"))
                        .font(regular(12.0))
                        .color(pal.muted),
                )
                .wrap(),
            );
        });
        let label = self.tr.t("can-label");
        let edit = egui::TextEdit::singleline(&mut self.can)
            .password(true)
            .char_limit(6)
            .font(mono(22.0))
            .text_color(pal.ink)
            .background_color(pal.surface)
            .desired_width(f32::INFINITY)
            .margin(Margin::symmetric(14, 12))
            .hint_text("······");
        ui.add(edit)
            .widget_info(|| WidgetInfo::labeled(WidgetType::TextEdit, true, &label));
        self.can.retain(|c| c.is_ascii_digit());
        ui.label(
            RichText::new(self.tr.t("can-note"))
                .font(regular(12.0))
                .color(pal.muted),
        );
    }

    // ------------------------------------------------------------ main panel

    fn main_panel(&mut self, ui: &mut egui::Ui) {
        let pal = *Palette::of(ui.ctx());
        let action = match &self.phase {
            Phase::Idle => {
                self.empty_view(ui, &pal);
                None
            }
            Phase::Reading { secure, progress } => {
                self.reading_view(ui, &pal, *secure, *progress);
                None
            }
            Phase::Failed(e) => self.error_view(ui, &pal, e),
            Phase::Done(outcome) => {
                let (discard, tab) = self.result_view(ui, &pal, outcome, self.tab);
                self.tab = tab;
                discard.then_some(Action::BackToStart)
            }
        };
        match action {
            Some(Action::EnterCanAgain) => {
                self.can.zeroize();
                self.phase = Phase::Idle;
            }
            Some(Action::BackToStart) => {
                self.phase = Phase::Idle;
                self.tab = Tab::Overview;
            }
            None => {}
        }
    }

    fn empty_view(&self, ui: &mut egui::Ui, pal: &Palette) {
        let tr = &self.tr;
        ui.vertical_centered(|ui| {
            ui.add_space((ui.available_height() * 0.12).clamp(24.0, 96.0));
            icons::paint(ui, vec2(180.0, 120.0), |p, r| {
                icons::reader_with_card(p, r, pal)
            });
            ui.add_space(20.0);
            ui.label(
                RichText::new(tr.t("empty-heading"))
                    .font(bold(28.0))
                    .color(pal.ink),
            );
            ui.add_space(6.0);
            ui.scope(|ui| {
                ui.set_max_width(520.0);
                ui.label(
                    RichText::new(tr.t("empty-text"))
                        .font(regular(15.0))
                        .color(pal.muted),
                );
            });
            ui.add_space(20.0);
            let width = ui.available_width().min(620.0);
            let infos = [
                ("empty-contact-title", "empty-contact-text"),
                ("empty-nfc-title", "empty-nfc-text"),
            ];
            let info_card = |ui: &mut egui::Ui, title: &str, text: &str| {
                card(pal).inner_margin(Margin::same(18)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.with_layout(Layout::top_down(Align::Min), |ui| {
                        ui.label(
                            RichText::new(tr.t(title))
                                .font(semibold(14.0))
                                .color(pal.ink),
                        );
                        ui.label(
                            RichText::new(tr.t(text))
                                .font(regular(13.0))
                                .color(pal.muted),
                        );
                    });
                });
            };
            ui.allocate_ui_with_layout(vec2(width, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.set_width(width);
                if width >= 520.0 {
                    ui.columns(2, |cols| {
                        for (col, (title, text)) in cols.iter_mut().zip(infos) {
                            info_card(col, title, text);
                        }
                    });
                } else {
                    for (title, text) in infos {
                        info_card(ui, title, text);
                    }
                }
            });
        });
    }

    fn reading_view(&self, ui: &mut egui::Ui, pal: &Palette, secure: bool, progress: Progress) {
        let tr = &self.tr;
        let steps: &[&str] = if secure {
            &[
                "step-connecting",
                "step-pace",
                "step-reading-secure",
                "step-parsing",
            ]
        } else {
            &["step-connecting", "step-reading", "step-parsing"]
        };
        let active = match progress {
            Progress::Starting | Progress::Connecting => 0,
            Progress::Reading | Progress::Pace => 1,
            Progress::ReadingSecure => 2,
            Progress::Parsing => steps.len() - 1,
        };
        centered_column(ui, 460.0, |ui| {
            card(pal)
                .inner_margin(Margin::same(28))
                .corner_radius(CornerRadius::same(14))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new(tr.t("reading-heading"))
                            .font(bold(20.0))
                            .color(pal.ink),
                    );
                    ui.add_space(2.0);
                    let hint = if secure {
                        "reading-hint-secure"
                    } else {
                        "reading-hint-contact"
                    };
                    ui.label(
                        RichText::new(tr.t(hint))
                            .font(regular(14.0))
                            .color(pal.muted),
                    );
                    ui.add_space(16.0);
                    for (i, key) in steps.iter().enumerate() {
                        let done = i < active;
                        let current = i == active;
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 12.0;
                            let bg = if done {
                                pal.accent
                            } else if current {
                                pal.accent_soft
                            } else {
                                pal.neutral_soft
                            };
                            let dot = icons::paint(ui, vec2(26.0, 26.0), |p, rect| {
                                p.circle_filled(rect.center(), 13.0, bg);
                                if done {
                                    let inner = egui::Rect::from_center_size(
                                        rect.center(),
                                        Vec2::splat(14.0),
                                    );
                                    icons::check(p, inner, pal.on_accent);
                                }
                            });
                            if current {
                                let inner = egui::Rect::from_center_size(
                                    dot.rect.center(),
                                    Vec2::splat(16.0),
                                );
                                egui::Spinner::new()
                                    .size(16.0)
                                    .color(pal.accent)
                                    .paint_at(ui, inner);
                            }
                            let font = if current {
                                semibold(15.0)
                            } else {
                                regular(15.0)
                            };
                            let ink = if done || current { pal.ink } else { pal.faint };
                            ui.label(RichText::new(tr.t(key)).font(font).color(ink));
                        });
                        ui.add_space(6.0);
                    }
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new(tr.t("reading-keep"))
                            .font(regular(13.0))
                            .color(pal.muted),
                    );
                });
        });
    }

    fn error_view(&self, ui: &mut egui::Ui, pal: &Palette, e: &ReadError) -> Option<Action> {
        let tr = &self.tr;
        let (title, text, action) = match e {
            ReadError::CanRequired => (
                tr.t("error-can-required-title"),
                tr.t("error-can-required-text"),
                Action::BackToStart,
            ),
            ReadError::WrongCan => (
                tr.t("error-wrong-can-title"),
                tr.t("error-wrong-can-text"),
                Action::EnterCanAgain,
            ),
            ReadError::CardRemoved => (
                tr.t("error-card-removed-title"),
                tr.t("error-card-removed-text"),
                Action::BackToStart,
            ),
            ReadError::ReaderGone => (
                tr.t("error-reader-gone-title"),
                tr.t("error-reader-gone-text"),
                Action::BackToStart,
            ),
            ReadError::NoCard => (
                tr.t("error-no-card-title"),
                tr.t("error-no-card-text"),
                Action::BackToStart,
            ),
            ReadError::Unsupported => (
                tr.t("error-unsupported-title"),
                tr.t("error-unsupported-text"),
                Action::BackToStart,
            ),
            ReadError::ServiceUnavailable => (
                tr.t("error-service-title"),
                tr.t("error-service-text"),
                Action::BackToStart,
            ),
            ReadError::Card(detail) => (
                tr.t("error-card-title"),
                tr.t_with("error-card-text", &[("detail", detail.clone().into())]),
                Action::BackToStart,
            ),
        };
        let button_label = match action {
            Action::EnterCanAgain => tr.t("error-retry-can"),
            Action::BackToStart => tr.t("error-back"),
        };
        let mut clicked = false;
        centered_column(ui, 480.0, |ui| {
            Frame::new()
                .fill(pal.surface)
                .stroke(Stroke::new(1.0, pal.error_border))
                .corner_radius(CornerRadius::same(14))
                .inner_margin(Margin::same(28))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = 14.0;
                    icons::paint(ui, vec2(44.0, 44.0), |p, rect| {
                        p.circle_filled(rect.center(), 22.0, pal.error_bg);
                        let inner = egui::Rect::from_center_size(rect.center(), Vec2::splat(22.0));
                        icons::alert(p, inner, pal.error_ink);
                    });
                    ui.label(RichText::new(&title).font(bold(20.0)).color(pal.ink));
                    ui.label(RichText::new(&text).font(regular(15.0)).color(pal.muted));
                    clicked = button(ui, pal, &button_label, Kind::Primary, true, false).clicked();
                });
        });
        clicked.then_some(action)
    }

    /// Returns whether the user discarded the data, and the selected tab.
    fn result_view(
        &self,
        ui: &mut egui::Ui,
        pal: &Palette,
        outcome: &ReadOutcome,
        mut tab: Tab,
    ) -> (bool, Tab) {
        let tr = &self.tr;
        let p = &outcome.vsd.personal;
        let i = &outcome.vsd.insurance;
        let mut discard = false;
        ui.spacing_mut().item_spacing.y = 18.0;

        if outcome.vsd.status.update_incomplete {
            Frame::new()
                .fill(pal.warn_bg)
                .stroke(Stroke::new(1.0, pal.warn_border))
                .corner_radius(CornerRadius::same(10))
                .inner_margin(Margin::symmetric(16, 14))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 12.0;
                        icons::paint(ui, vec2(20.0, 20.0), |pa, r| {
                            icons::info(pa, r, pal.warn_ink)
                        });
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            ui.label(
                                RichText::new(tr.t("update-incomplete-title"))
                                    .font(semibold(14.0))
                                    .color(pal.warn_ink),
                            );
                            ui.label(
                                RichText::new(tr.t("update-incomplete-text"))
                                    .font(regular(14.0))
                                    .color(pal.warn_ink),
                            );
                        });
                    });
                });
        }

        // Avatar, name, insurer and insurant ID; badge and discard on the right,
        // or on their own row when the window is narrow.
        let (via, secure) = match (outcome.interface, outcome.secured) {
            (CardInterface::Contactless, _) => (tr.t("via-contactless"), true),
            (_, true) => (tr.t("via-contact-pace"), true),
            _ => (tr.t("via-contact"), false),
        };
        let wide = ui.available_width() >= 760.0;
        // Badge and discard button; in a right-to-left row they end up at the
        // right edge, in a left-to-right row at the left.
        let mut actions = |ui: &mut egui::Ui, rtl: bool| {
            ui.spacing_mut().item_spacing.x = 10.0;
            if !rtl {
                badge(ui, pal, &via, secure);
            }
            discard |= button(
                ui,
                pal,
                &tr.t("button-discard"),
                Kind::Secondary,
                true,
                false,
            )
            .clicked();
            if rtl {
                badge(ui, pal, &via, secure);
            }
        };
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 16.0;
            let initials = initials(p);
            icons::paint(ui, vec2(56.0, 56.0), |pa, rect| {
                pa.circle_filled(rect.center(), 28.0, pal.accent_soft);
                pa.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &initials,
                    bold(20.0),
                    pal.accent_text,
                );
            });
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                let name = join(&[&p.first_name, &p.last_name]).unwrap_or_default();
                let row = ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    ui.label(RichText::new(&name).font(bold(26.0)).color(pal.ink));
                    copy_slot(ui)
                });
                let show = ui.rect_contains_pointer(row.response.rect);
                finish_copy(ui, pal, tr, row.inner, show, "copy-tooltip", || {
                    name.clone()
                });
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 14.0;
                    if let Some(payer) = &i.payer_name {
                        ui.label(RichText::new(payer).font(regular(14.0)).color(pal.muted));
                    }
                    if let Some(id) = &p.insurant_id {
                        let row = ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            ui.label(
                                RichText::new(tr.t("insurant-id-inline"))
                                    .font(regular(14.0))
                                    .color(pal.muted),
                            );
                            ui.label(RichText::new(id).font(mono(14.0)).color(pal.ink));
                            copy_slot(ui)
                        });
                        let show = ui.rect_contains_pointer(row.response.rect);
                        finish_copy(ui, pal, tr, row.inner, show, "copy-tooltip", || id.clone());
                    }
                });
            });
            if wide {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| actions(ui, true));
            }
        });
        if !wide {
            ui.horizontal(|ui| actions(ui, false));
        }

        // Tabs with a baseline under the whole row.
        let tabs_rect = ui
            .horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 24.0;
                for (t, key) in [
                    (Tab::Overview, "tab-overview"),
                    (Tab::Personal, "tab-personal"),
                    (Tab::Insurance, "tab-insurance"),
                    (Tab::Raw, "tab-raw"),
                ] {
                    if tab_button(ui, pal, &tr.t(key), tab == t).clicked() {
                        tab = t;
                    }
                }
            })
            .response
            .rect;
        ui.painter().hline(
            ui.max_rect().x_range(),
            tabs_rect.bottom(),
            Stroke::new(1.0, pal.line),
        );

        match tab {
            Tab::Overview => {
                let personal: Vec<Row> = personal_rows(tr, p)
                    .into_iter()
                    .filter(|r| r.overview)
                    .collect();
                let insurance: Vec<Row> = insurance_rows(tr, i)
                    .into_iter()
                    .filter(|r| r.overview)
                    .collect();
                let cards = [
                    (tr.t("section-personal"), personal),
                    (tr.t("section-insurance"), insurance),
                ];
                if ui.available_width() >= 640.0 {
                    ui.columns(2, |cols| {
                        for (col, (title, rows)) in cols.iter_mut().zip(cards.iter()) {
                            summary_card(col, pal, tr, title, rows);
                        }
                    });
                } else {
                    for (title, rows) in &cards {
                        summary_card(ui, pal, tr, title, rows);
                    }
                }
            }
            Tab::Personal | Tab::Insurance => {
                let (title, rows) = if tab == Tab::Personal {
                    (tr.t("section-personal"), personal_rows(tr, p))
                } else {
                    (tr.t("section-insurance"), insurance_rows(tr, i))
                };
                card(pal)
                    .inner_margin(Margin::symmetric(20, 16))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        definition_list(ui, pal, tr, &title, &rows);
                    });
            }
            Tab::Raw => {
                card(pal).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    raw_table(ui, pal, tr, &p.xml, &i.xml);
                });
            }
        }

        ui.label(
            RichText::new(tr.t("result-footer"))
                .font(regular(12.0))
                .color(pal.muted),
        );
        (discard, tab)
    }
}

impl eframe::App for App {
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_events();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.draw(ui);
    }
}

impl App {
    /// Draws the whole window.
    pub fn draw(&mut self, ui: &mut egui::Ui) {
        let pal = *Palette::of(ui.ctx());
        egui::Panel::top("header")
            .frame(
                Frame::new()
                    .fill(pal.surface)
                    .inner_margin(Margin::symmetric(24, 14)),
            )
            .show(ui, |ui| self.header(ui));
        egui::Panel::left("readers")
            .resizable(false)
            .exact_size(330.0)
            .frame(
                Frame::new()
                    .fill(pal.sidebar)
                    .inner_margin(Margin::same(20)),
            )
            .show(ui, |ui| self.sidebar(ui));
        egui::CentralPanel::default_margins()
            .frame(
                Frame::new()
                    .fill(pal.ground)
                    .inner_margin(Margin::symmetric(32, 28)),
            )
            .show(ui, |ui| {
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.main_panel(ui));
            });
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.can.zeroize();
    }
}

#[cfg(test)]
#[path = "preview.rs"]
mod preview;

// ------------------------------------------------------------------ widgets

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Primary,
    Secondary,
}

/// A filled (primary) or outlined (secondary) button, 40 px high.
fn button(
    ui: &mut egui::Ui,
    pal: &Palette,
    text: &str,
    kind: Kind,
    enabled: bool,
    full_width: bool,
) -> egui::Response {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), semibold(14.0), Color32::PLACEHOLDER);
    let width = if full_width {
        ui.available_width()
    } else {
        galley.size().x + 32.0
    };
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(width, 40.0), sense);
    let hovered = enabled && response.hovered();
    let (fill, stroke, ink) = match (kind, enabled) {
        (_, false) => (pal.disabled_bg, Stroke::NONE, pal.disabled_fg),
        (Kind::Primary, true) => (
            if hovered {
                pal.accent_hover
            } else {
                pal.accent
            },
            Stroke::NONE,
            pal.on_accent,
        ),
        (Kind::Secondary, true) => (
            if hovered {
                pal.neutral_soft
            } else {
                pal.surface
            },
            Stroke::new(1.0, pal.control_border),
            pal.ink,
        ),
    };
    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(8),
        fill,
        stroke,
        StrokeKind::Inside,
    );
    if response.has_focus() {
        painter.rect_stroke(
            rect.expand(3.0),
            CornerRadius::same(10),
            Stroke::new(2.0, pal.accent),
            StrokeKind::Outside,
        );
    }
    painter.galley(rect.center() - galley.size() / 2.0, galley, ink);
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, text));
    if enabled {
        response.on_hover_cursor(CursorIcon::PointingHand)
    } else {
        response
    }
}

fn tab_button(ui: &mut egui::Ui, pal: &Palette, text: &str, selected: bool) -> egui::Response {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), semibold(14.0), Color32::PLACEHOLDER);
    let (rect, response) =
        ui.allocate_exact_size(vec2(galley.size().x + 4.0, 44.0), Sense::click());
    let ink = if selected {
        pal.accent_text
    } else if response.hovered() {
        pal.ink
    } else {
        pal.muted
    };
    let painter = ui.painter();
    painter.galley(rect.center() - galley.size() / 2.0, galley, ink);
    if selected {
        painter.hline(
            rect.x_range(),
            rect.bottom() - 1.0,
            Stroke::new(2.0, pal.accent),
        );
    }
    if response.has_focus() {
        painter.rect_stroke(
            rect,
            CornerRadius::same(4),
            Stroke::new(2.0, pal.accent),
            StrokeKind::Outside,
        );
    }
    response
        .widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, true, selected, text));
    response.on_hover_cursor(CursorIcon::PointingHand)
}

fn badge(ui: &mut egui::Ui, pal: &Palette, text: &str, secure: bool) {
    // Painted as one widget so it keeps its order inside right-to-left rows.
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), semibold(13.0), pal.accent_text);
    let icon = 14.0;
    let size = vec2(12.0 + icon + 6.0 + galley.size().x + 12.0, 30.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(255), pal.accent_soft);
    let icon_rect = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 12.0, rect.center().y - icon / 2.0),
        Vec2::splat(icon),
    );
    if secure {
        icons::lock(painter, icon_rect, pal.accent_text);
    } else {
        icons::contact(painter, icon_rect, pal.accent_text);
    }
    let text_pos = egui::pos2(
        icon_rect.right() + 6.0,
        rect.center().y - galley.size().y / 2.0,
    );
    painter.galley(text_pos, galley, pal.accent_text);
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, text));
}

fn card(pal: &Palette) -> Frame {
    Frame::new()
        .fill(pal.surface)
        .stroke(Stroke::new(1.0, pal.line))
        .corner_radius(CornerRadius::same(12))
}

/// A column of at most `max_width`, centered horizontally and placed a bit
/// below the top.
fn centered_column(ui: &mut egui::Ui, max_width: f32, add: impl FnOnce(&mut egui::Ui)) {
    ui.vertical_centered(|ui| {
        ui.add_space((ui.available_height() * 0.18).clamp(16.0, 140.0));
        ui.scope(|ui| {
            ui.set_max_width(max_width.min(ui.available_width()));
            ui.with_layout(Layout::top_down(Align::Min), add);
        });
    });
}

/// Reserves an 18×18 copy button in the current row. Call [`finish_copy`]
/// once the row is laid out, so it can show the button while the row is hovered.
fn copy_slot(ui: &mut egui::Ui) -> egui::Response {
    ui.allocate_exact_size(Vec2::splat(18.0), Sense::click()).1
}

/// Paints the copy button while `show` (and briefly after copying, as a check
/// mark) and puts `text` on the clipboard when it is clicked.
fn finish_copy(
    ui: &egui::Ui,
    pal: &Palette,
    tr: &I18n,
    slot: egui::Response,
    show: bool,
    tooltip: &str,
    text: impl FnOnce() -> String,
) {
    let ctx = ui.ctx();
    let now = ui.input(|i| i.time);
    if slot.clicked() {
        ctx.copy_text(text());
        ctx.data_mut(|d| d.insert_temp(slot.id, now));
    }
    let copied_at: Option<f64> = ctx.data(|d| d.get_temp(slot.id));
    let copied = copied_at.is_some_and(|t| now - t < 1.5);
    if copied {
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
    }
    if !(show || copied || slot.has_focus()) {
        return;
    }
    let painter = ui.painter();
    if copied {
        icons::check(painter, slot.rect.shrink(2.0), pal.accent);
    } else {
        let ink = if slot.hovered() { pal.ink } else { pal.faint };
        icons::copy(painter, slot.rect, ink);
    }
    let label = if copied {
        tr.t("copy-done")
    } else {
        tr.t(tooltip)
    };
    slot.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &label));
    slot.on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(label);
}

/// Text for "copy all" of a card: the title, then one `label: value` line per row.
fn rows_as_text(title: &str, rows: &[Row]) -> String {
    let mut out = format!("{title}\n");
    for r in rows {
        out.push_str(&format!("{}: {}\n", r.label, r.value));
    }
    out
}

/// Card title with a "copy all" button at the right.
fn card_title(
    ui: &mut egui::Ui,
    pal: &Palette,
    tr: &I18n,
    title: &str,
    text: impl FnOnce() -> String,
) {
    let slot = ui
        .horizontal(|ui| {
            ui.label(RichText::new(title).font(semibold(15.0)).color(pal.ink));
            ui.with_layout(Layout::right_to_left(Align::Center), copy_slot)
                .inner
        })
        .inner;
    finish_copy(ui, pal, tr, slot, true, "copy-all-tooltip", text);
}

/// One `label  value [copy]` line; the copy button appears while hovered.
fn value_row(ui: &mut egui::Ui, pal: &Palette, tr: &I18n, r: &Row, label_width: f32) {
    let row = ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.allocate_ui_with_layout(vec2(label_width, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_width(label_width);
            ui.label(RichText::new(&r.label).font(regular(14.0)).color(pal.muted));
        });
        let max_value = (ui.available_width() - 26.0).max(40.0);
        ui.scope(|ui| {
            ui.set_max_width(max_value);
            ui.add(egui::Label::new(value_text(r, pal)).wrap());
        });
        copy_slot(ui)
    });
    let show = ui.rect_contains_pointer(row.response.rect);
    finish_copy(ui, pal, tr, row.inner, show, "copy-tooltip", || {
        r.value.clone()
    });
}

fn summary_card(ui: &mut egui::Ui, pal: &Palette, tr: &I18n, title: &str, rows: &[Row]) {
    card(pal).inner_margin(Margin::same(20)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        card_title(ui, pal, tr, title, || rows_as_text(title, rows));
        ui.add_space(4.0);
        ui.spacing_mut().item_spacing.y = 10.0;
        let label_width = ui.available_width() * 0.36;
        for r in rows {
            value_row(ui, pal, tr, r, label_width);
        }
    });
}

#[derive(Clone)]
struct Row {
    label: String,
    value: String,
    mono: bool,
    /// Shown on the overview card.
    overview: bool,
}

fn row(label: String, value: Option<String>, mono: bool, overview: bool) -> Option<Row> {
    value.map(|value| Row {
        label,
        value,
        mono,
        overview,
    })
}

fn value_text(r: &Row, pal: &Palette) -> RichText {
    let font = if r.mono { mono(14.0) } else { medium(14.0) };
    RichText::new(&r.value).font(font).color(pal.ink)
}

fn definition_list(ui: &mut egui::Ui, pal: &Palette, tr: &I18n, title: &str, rows: &[Row]) {
    card_title(ui, pal, tr, title, || rows_as_text(title, rows));
    let label_width = (ui.available_width() * 0.33).max(140.0);
    let count = rows.len();
    ui.spacing_mut().item_spacing.y = 0.0;
    for (n, r) in rows.iter().enumerate() {
        ui.add_space(12.0);
        value_row(ui, pal, tr, r, label_width);
        ui.add_space(12.0);
        if n + 1 < count {
            let y = ui.cursor().top();
            ui.painter()
                .hline(ui.max_rect().x_range(), y, Stroke::new(1.0, pal.line_soft));
        }
    }
}

/// Leaves of a document as `(path without the root element, value)`.
fn raw_leaves(root: &Element) -> Vec<(String, String)> {
    root.leaves()
        .into_iter()
        .map(|(path, value)| {
            let path = path
                .split_once('/')
                .map_or(path.as_str(), |(_, rest)| rest)
                .to_owned();
            (path, value)
        })
        .collect()
}

fn raw_table(ui: &mut egui::Ui, pal: &Palette, tr: &I18n, pd: &Element, vd: &Element) {
    let groups = [
        (tr.t("raw-group-pd"), raw_leaves(pd)),
        (tr.t("raw-group-vd"), raw_leaves(vd)),
    ];
    Frame::new()
        .inner_margin(Margin::symmetric(20, 14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let slot = ui
                .horizontal_top(|ui| {
                    ui.scope(|ui| {
                        ui.set_max_width((ui.available_width() - 30.0).max(100.0));
                        ui.add(
                            egui::Label::new(
                                RichText::new(tr.t("raw-intro"))
                                    .font(regular(13.0))
                                    .color(pal.muted),
                            )
                            .wrap(),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Min), copy_slot)
                        .inner
                })
                .inner;
            finish_copy(ui, pal, tr, slot, true, "copy-all-tooltip", || {
                let mut out = String::new();
                for (caption, leaves) in &groups {
                    out.push_str(&format!("{caption}\n"));
                    for (path, value) in leaves {
                        out.push_str(&format!("{path}\t{value}\n"));
                    }
                }
                out
            });
        });
    ScrollArea::horizontal().id_salt("raw").show(ui, |ui| {
        Frame::new()
            .inner_margin(Margin {
                left: 20,
                right: 20,
                top: 4,
                bottom: 16,
            })
            .show(ui, |ui| {
                ui.set_min_width(560.0);
                egui::Grid::new("raw_table")
                    .num_columns(2)
                    .spacing([32.0, 9.0])
                    .striped(true)
                    .show(ui, |ui| {
                        for key in ["raw-col-path", "raw-col-value"] {
                            ui.label(
                                RichText::new(tr.t(key))
                                    .font(semibold(13.0))
                                    .color(pal.muted),
                            );
                        }
                        ui.end_row();
                        for (caption, leaves) in &groups {
                            ui.label(RichText::new(caption).font(semibold(13.0)).color(pal.ink));
                            ui.label("");
                            ui.end_row();
                            for (path, value) in leaves {
                                ui.label(RichText::new(path).font(mono(13.0)).color(pal.muted));
                                ui.label(RichText::new(value).font(mono(13.0)).color(pal.ink));
                                ui.end_row();
                            }
                        }
                    });
            });
    });
}

// --------------------------------------------------------------- formatting

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

fn initials(p: &PersonalData) -> String {
    [&p.first_name, &p.last_name]
        .iter()
        .filter_map(|n| n.as_deref().and_then(|s| s.chars().next()))
        .flat_map(char::to_uppercase)
        .collect()
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

fn personal_rows(tr: &I18n, p: &PersonalData) -> Vec<Row> {
    let name = join(&[
        &p.title,
        &p.first_name,
        &p.name_prefix,
        &p.last_name,
        &p.name_addition,
    ]);
    let mut rows = vec![
        row(tr.t("field-name"), name, false, true),
        row(
            tr.t("field-birth-date"),
            date(tr, &p.birth_date),
            false,
            true,
        ),
        row(tr.t("field-sex"), sex(tr, &p.sex), false, true),
        row(tr.t("field-insurant-id"), p.insurant_id.clone(), true, true),
    ];
    if let Some(a) = &p.street_address {
        rows.push(row(
            tr.t("field-street"),
            join(&[&a.street, &a.house_number]),
            false,
            false,
        ));
        rows.push(row(
            tr.t("field-address-addition"),
            a.addition.clone(),
            false,
            false,
        ));
        rows.push(row(
            tr.t("field-city"),
            join(&[&a.postal_code, &a.city]),
            false,
            false,
        ));
        rows.push(row(tr.t("field-country"), a.country.clone(), false, false));
    }
    if let Some(a) = &p.postbox_address {
        rows.push(row(tr.t("field-postbox"), a.postbox.clone(), false, false));
        rows.push(row(
            tr.t("field-postbox-city"),
            join(&[&a.postal_code, &a.city]),
            false,
            false,
        ));
    }
    rows.into_iter().flatten().collect()
}

fn insurance_rows(tr: &I18n, i: &InsuranceData) -> Vec<Row> {
    let end = date(tr, &i.coverage_end).or_else(|| Some(tr.t("coverage-open-ended")));
    [
        row(tr.t("field-payer"), i.payer_name.clone(), false, true),
        row(tr.t("field-payer-id"), i.payer_id.clone(), true, false),
        row(
            tr.t("field-billing-payer"),
            i.billing_payer_name.clone(),
            false,
            false,
        ),
        row(
            tr.t("field-billing-payer-id"),
            i.billing_payer_id.clone(),
            true,
            false,
        ),
        row(
            tr.t("field-coverage-start"),
            date(tr, &i.coverage_start),
            false,
            true,
        ),
        row(tr.t("field-coverage-end"), end, false, true),
        row(
            tr.t("field-insurant-type"),
            insurant_type(tr, &i.insurant_type),
            false,
            true,
        ),
        row(
            tr.t("field-special-group"),
            i.special_group.clone(),
            false,
            false,
        ),
        row(tr.t("field-dmp"), i.dmp.clone(), false, false),
        row(tr.t("field-wop"), i.wop.clone(), true, false),
        row(tr.t("field-schema"), i.schema_version.clone(), true, false),
    ]
    .into_iter()
    .flatten()
    .collect()
}

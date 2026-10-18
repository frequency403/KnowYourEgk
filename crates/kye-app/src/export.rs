//! Export of the read data as plain text, JSON or PDF.
//!
//! Text and PDF use the labels of the current UI language; JSON uses stable
//! English keys and the values as stored on the card, for further processing.

use printpdf::{
    Color, Line, LinePoint, Mm, Op, ParsedFont, PdfDocument, PdfFontHandle, PdfPage,
    PdfSaveOptions, Point, Pt, Rgb, TextItem,
};
use serde_json::{Map, Value, json};

use kye_egk::Vsd;
use kye_egk::xml::Element;

use crate::theme;

/// Export formats offered in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Text,
    Json,
    Pdf,
}

impl Format {
    pub const ALL: [Format; 3] = [Format::Pdf, Format::Text, Format::Json];

    pub fn extension(self) -> &'static str {
        match self {
            Format::Text => "txt",
            Format::Json => "json",
            Format::Pdf => "pdf",
        }
    }
}

/// A titled block of `label: value` lines.
pub struct Section {
    pub title: String,
    pub rows: Vec<(String, String)>,
}

/// Everything that goes into a text or PDF export, already translated.
pub struct Document {
    pub title: String,
    /// Short lines under the title (how the card was read, …).
    pub meta: Vec<String>,
    /// Highlighted note, e.g. an incomplete last update.
    pub warning: Option<String>,
    pub sections: Vec<Section>,
    /// Caption of the raw-fields part and its groups of `(path, value)`.
    pub raw_title: String,
    pub raw: Vec<(String, Vec<(String, String)>)>,
    pub footer: String,
}

/// Plain UTF-8 text.
pub fn to_text(doc: &Document) -> String {
    let mut out = String::new();
    out.push_str(&doc.title);
    out.push('\n');
    out.push_str(&"=".repeat(doc.title.chars().count()));
    out.push('\n');
    for m in &doc.meta {
        out.push_str(m);
        out.push('\n');
    }
    if let Some(w) = &doc.warning {
        out.push_str(&format!("\n! {w}\n"));
    }
    for s in &doc.sections {
        out.push_str(&format!(
            "\n{}\n{}\n",
            s.title,
            "-".repeat(s.title.chars().count())
        ));
        let width = s
            .rows
            .iter()
            .map(|(l, _)| l.chars().count())
            .max()
            .unwrap_or(0);
        for (label, value) in &s.rows {
            let pad = width - label.chars().count();
            out.push_str(&format!("{label}:{}  {value}\n", " ".repeat(pad)));
        }
    }
    out.push_str(&format!(
        "\n{}\n{}\n",
        doc.raw_title,
        "-".repeat(doc.raw_title.chars().count())
    ));
    for (caption, leaves) in &doc.raw {
        out.push_str(&format!("\n[{caption}]\n"));
        for (path, value) in leaves {
            out.push_str(&format!("{path} = {value}\n"));
        }
    }
    out.push_str(&format!("\n{}\n", doc.footer));
    out
}

fn raw_object(root: &Element) -> Value {
    let mut map = Map::new();
    for (path, value) in root.leaves() {
        map.insert(path, Value::String(value));
    }
    Value::Object(map)
}

fn iso_date(v: &Option<String>) -> Value {
    match v.as_deref() {
        Some(d) if d.len() == 8 && d.bytes().all(|b| b.is_ascii_digit()) => {
            Value::String(format!("{}-{}-{}", &d[0..4], &d[4..6], &d[6..8]))
        }
        Some(d) => Value::String(d.to_owned()),
        None => Value::Null,
    }
}

/// Language-independent JSON with the values as stored on the card.
pub fn to_json(vsd: &Vsd, read_via: &str, secured: bool) -> String {
    let p = &vsd.personal;
    let i = &vsd.insurance;
    let s = &vsd.status;
    let address = p.street_address.as_ref().map(|a| {
        json!({
            "street": a.street,
            "house_number": a.house_number,
            "addition": a.addition,
            "postal_code": a.postal_code,
            "city": a.city,
            "country": a.country,
        })
    });
    let postbox = p.postbox_address.as_ref().map(|a| {
        json!({
            "postbox": a.postbox,
            "postal_code": a.postal_code,
            "city": a.city,
            "country": a.country,
        })
    });
    let ts = &s.timestamp;
    let value = json!({
        "generator": format!("KnowYourEgk {}", env!("CARGO_PKG_VERSION")),
        "read_via": read_via,
        "secure_messaging": secured,
        "status": {
            "update_incomplete": s.update_incomplete,
            "last_update_utc": format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
                ts.year, ts.month, ts.day, ts.hour, ts.minute, ts.second
            ),
            "version_xml": s.version_xml.to_string(),
            "version_storage": s.version_storage.to_string(),
        },
        "personal": {
            "schema_version": p.schema_version,
            "insurant_id": p.insurant_id,
            "birth_date": iso_date(&p.birth_date),
            "first_name": p.first_name,
            "last_name": p.last_name,
            "sex": p.sex,
            "name_prefix": p.name_prefix,
            "name_addition": p.name_addition,
            "title": p.title,
            "street_address": address,
            "postbox_address": postbox,
        },
        "insurance": {
            "schema_version": i.schema_version,
            "coverage_start": iso_date(&i.coverage_start),
            "coverage_end": iso_date(&i.coverage_end),
            "payer_id": i.payer_id,
            "payer_name": i.payer_name,
            "payer_country": i.payer_country,
            "billing_payer_id": i.billing_payer_id,
            "billing_payer_name": i.billing_payer_name,
            "insurant_type": i.insurant_type,
            "special_group": i.special_group,
            "dmp": i.dmp,
            "wop": i.wop,
        },
        "raw": {
            "pd": raw_object(&p.xml),
            "vd": raw_object(&i.xml),
        },
    });
    // Serialising a `Value` built from strings cannot fail; fall back to the
    // compact form just in case.
    serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string())
}

// --------------------------------------------------------------------- PDF

const PAGE_W: f32 = 595.28;
const PAGE_H: f32 = 841.89;
const MARGIN: f32 = 56.0;
const LABEL_X: f32 = MARGIN;
const VALUE_X: f32 = MARGIN + 170.0;

struct Fonts {
    regular: (ParsedFont, PdfFontHandle),
    semibold: (ParsedFont, PdfFontHandle),
    bold: (ParsedFont, PdfFontHandle),
    mono: (ParsedFont, PdfFontHandle),
}

#[derive(Clone, Copy)]
enum Face {
    Regular,
    SemiBold,
    Bold,
    Mono,
}

impl Fonts {
    fn get(&self, f: Face) -> &(ParsedFont, PdfFontHandle) {
        match f {
            Face::Regular => &self.regular,
            Face::SemiBold => &self.semibold,
            Face::Bold => &self.bold,
            Face::Mono => &self.mono,
        }
    }

    /// Advance width of `text` in pt.
    fn width(&self, f: Face, size: f32, text: &str) -> f32 {
        let font = &self.get(f).0;
        let upm = f32::from(font.units_per_em.max(1));
        let units: f32 = text
            .chars()
            .map(|c| {
                font.lookup_glyph_index(u32::from(c))
                    .and_then(|g| font.get_glyph_width(g))
                    .map_or(upm * 0.5, f32::from)
            })
            .sum();
        units / upm * size
    }

    /// Greedy word wrap to `max` pt; very long words are split by characters.
    fn wrap(&self, f: Face, size: f32, text: &str, max: f32) -> Vec<String> {
        let mut lines = Vec::new();
        let mut line = String::new();
        for word in text.split(' ') {
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if self.width(f, size, &candidate) <= max {
                line = candidate;
                continue;
            }
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            let mut chunk = String::new();
            for c in word.chars() {
                chunk.push(c);
                if self.width(f, size, &chunk) > max {
                    chunk.pop();
                    lines.push(std::mem::take(&mut chunk));
                    chunk.push(c);
                }
            }
            line = chunk;
        }
        if !line.is_empty() || lines.is_empty() {
            lines.push(line);
        }
        lines
    }
}

fn rgb(hex: u32) -> Color {
    Color::Rgb(Rgb::new(
        f32::from((hex >> 16) as u8) / 255.0,
        f32::from((hex >> 8) as u8) / 255.0,
        f32::from(hex as u8) / 255.0,
        None,
    ))
}

const INK: u32 = 0x17201D;
const MUTED: u32 = 0x55605B;
const ACCENT: u32 = 0x0B6B63;
const WARN: u32 = 0x6B3B00;
const LINE: u32 = 0xDDE1DC;

/// Lays out text top-down and starts new pages as needed.
struct Writer<'a> {
    fonts: &'a Fonts,
    pages: Vec<Vec<Op>>,
    ops: Vec<Op>,
    y: f32,
}

impl<'a> Writer<'a> {
    fn new(fonts: &'a Fonts) -> Self {
        Self {
            fonts,
            pages: Vec::new(),
            ops: Vec::new(),
            y: PAGE_H - MARGIN,
        }
    }

    fn ensure(&mut self, height: f32) {
        if self.y - height < MARGIN {
            self.pages.push(std::mem::take(&mut self.ops));
            self.y = PAGE_H - MARGIN;
        }
    }

    fn text_at(&mut self, face: Face, size: f32, color: u32, x: f32, y: f32, text: &str) {
        let handle = self.fonts.get(face).1.clone();
        self.ops.extend([
            Op::StartTextSection,
            Op::SetFont {
                font: handle,
                size: Pt(size),
            },
            Op::SetFillColor { col: rgb(color) },
            Op::SetTextCursor {
                pos: Point { x: Pt(x), y: Pt(y) },
            },
            Op::ShowText {
                items: vec![TextItem::Text(text.to_owned())],
            },
            Op::EndTextSection,
        ]);
    }

    /// A paragraph across the full width.
    fn paragraph(&mut self, face: Face, size: f32, color: u32, text: &str, gap_after: f32) {
        let lead = size * 1.4;
        for line in self.fonts.wrap(face, size, text, PAGE_W - 2.0 * MARGIN) {
            self.ensure(lead);
            self.y -= lead;
            self.text_at(face, size, color, MARGIN, self.y, &line);
        }
        self.y -= gap_after;
    }

    fn rule(&mut self) {
        self.ensure(8.0);
        self.y -= 6.0;
        self.ops.extend([
            Op::SetOutlineColor { col: rgb(LINE) },
            Op::SetOutlineThickness { pt: Pt(0.75) },
            Op::DrawLine {
                line: Line {
                    points: vec![
                        LinePoint {
                            p: Point {
                                x: Pt(MARGIN),
                                y: Pt(self.y),
                            },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point {
                                x: Pt(PAGE_W - MARGIN),
                                y: Pt(self.y),
                            },
                            bezier: false,
                        },
                    ],
                    is_closed: false,
                },
            },
        ]);
        self.y -= 6.0;
    }

    /// `label    value` with the value wrapped in its column.
    fn row(
        &mut self,
        label_x: f32,
        value_x: f32,
        label: &str,
        value: &str,
        value_face: Face,
        size: f32,
    ) {
        let lead = size * 1.45;
        let label_lines = self
            .fonts
            .wrap(Face::Regular, size, label, value_x - label_x - 12.0);
        let value_lines = self
            .fonts
            .wrap(value_face, size, value, PAGE_W - MARGIN - value_x);
        let n = label_lines.len().max(value_lines.len());
        self.ensure(lead * n as f32);
        for k in 0..n {
            self.y -= lead;
            if let Some(l) = label_lines.get(k) {
                self.text_at(Face::Regular, size, MUTED, label_x, self.y, l);
            }
            if let Some(v) = value_lines.get(k) {
                self.text_at(value_face, size, INK, value_x, self.y, v);
            }
        }
        self.y -= size * 0.35;
    }

    fn finish(mut self) -> Vec<Vec<Op>> {
        self.pages.push(self.ops);
        self.pages
    }
}

fn load_font(bytes: &[u8], doc: &mut PdfDocument) -> Result<(ParsedFont, PdfFontHandle), String> {
    let mut warnings = Vec::new();
    let font = ParsedFont::from_bytes(bytes, 0, &mut warnings)
        .ok_or_else(|| "could not load the embedded font".to_owned())?;
    let id = doc.add_font(&font);
    Ok((font, PdfFontHandle::External(id)))
}

/// A4 PDF with embedded IBM Plex fonts.
pub fn to_pdf(doc: &Document) -> Result<Vec<u8>, String> {
    let mut pdf = PdfDocument::new(&doc.title);
    let fonts = Fonts {
        regular: load_font(theme::SANS_REGULAR, &mut pdf)?,
        semibold: load_font(theme::SANS_SEMIBOLD, &mut pdf)?,
        bold: load_font(theme::SANS_BOLD, &mut pdf)?,
        mono: load_font(theme::MONO_REGULAR, &mut pdf)?,
    };

    let mut w = Writer::new(&fonts);
    w.paragraph(Face::Bold, 20.0, INK, &doc.title, 4.0);
    for m in &doc.meta {
        w.paragraph(Face::Regular, 10.0, MUTED, m, 0.0);
    }
    w.y -= 8.0;
    if let Some(warning) = &doc.warning {
        w.paragraph(Face::SemiBold, 10.5, WARN, warning, 6.0);
    }
    for s in &doc.sections {
        w.y -= 10.0;
        w.paragraph(Face::SemiBold, 13.0, ACCENT, &s.title, 2.0);
        w.rule();
        for (label, value) in &s.rows {
            w.row(LABEL_X, VALUE_X, label, value, Face::Regular, 10.5);
        }
    }
    w.y -= 14.0;
    w.paragraph(Face::SemiBold, 13.0, ACCENT, &doc.raw_title, 2.0);
    w.rule();
    for (caption, leaves) in &doc.raw {
        w.y -= 4.0;
        w.paragraph(Face::SemiBold, 10.0, INK, caption, 2.0);
        for (path, value) in leaves {
            w.row(LABEL_X, LABEL_X + 290.0, path, value, Face::Mono, 8.5);
        }
    }
    w.y -= 12.0;
    w.paragraph(Face::Regular, 8.5, MUTED, &doc.footer, 0.0);

    let pages = w
        .finish()
        .into_iter()
        .map(|ops| PdfPage::new(Mm(210.0), Mm(297.0), ops))
        .collect();
    let mut warnings = Vec::new();
    Ok(pdf
        .with_pages(pages)
        .save(&PdfSaveOptions::default(), &mut warnings))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Document {
        Document {
            title: "Versichertenstammdaten".into(),
            meta: vec!["Kontaktlos gelesen · verschlüsselt".into()],
            warning: Some("Unvollständig".into()),
            sections: vec![Section {
                title: "Persönliche Daten".into(),
                rows: vec![
                    ("Name".into(), "Jürgen Testmann-Größe".into()),
                    ("Versicherten-ID".into(), "X110000000".into()),
                ],
            }],
            raw_title: "Alle Felder".into(),
            raw: vec![(
                "EF.PD".into(),
                vec![("Versicherter/Person/Vorname".into(), "Jürgen".into())],
            )],
            footer: "KnowYourEgk".into(),
        }
    }

    #[test]
    fn text_export() {
        let t = to_text(&sample());
        assert!(t.starts_with("Versichertenstammdaten\n======================\n"));
        assert!(t.contains("Name:             Jürgen Testmann-Größe\n"));
        assert!(t.contains("Versicherter/Person/Vorname = Jürgen\n"));
        assert!(t.contains("! Unvollständig"));
    }

    #[test]
    fn pdf_export() {
        let bytes = to_pdf(&sample()).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        assert!(bytes.len() > 1000);
    }

    #[test]
    fn wrapping() {
        let mut pdf = PdfDocument::new("t");
        let fonts = Fonts {
            regular: load_font(theme::SANS_REGULAR, &mut pdf).unwrap(),
            semibold: load_font(theme::SANS_SEMIBOLD, &mut pdf).unwrap(),
            bold: load_font(theme::SANS_BOLD, &mut pdf).unwrap(),
            mono: load_font(theme::MONO_REGULAR, &mut pdf).unwrap(),
        };
        let w = fonts.width(Face::Regular, 10.0, "Hello");
        assert!(w > 15.0 && w < 40.0, "{w}");
        let lines = fonts.wrap(Face::Regular, 10.0, "one two three four five six", 40.0);
        assert!(lines.len() > 1);
        assert!(
            lines
                .iter()
                .all(|l| fonts.width(Face::Regular, 10.0, l) <= 40.0)
        );
    }
}

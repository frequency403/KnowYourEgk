//! gzip + ISO-8859-15 + namespace-agnostic XML element tree.

use std::io::Read;

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::EgkError;

/// Upper bound for decompressed documents (EF.PD/EF.VD are < 2 KiB compressed).
const MAX_XML_LEN: u64 = 256 * 1024;

/// An XML element with its local name (namespace prefix stripped).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Element {
    pub name: String,
    pub attributes: Vec<(String, String)>,
    /// Concatenated, trimmed character data directly inside this element.
    pub text: String,
    pub children: Vec<Element>,
}

impl Element {
    /// First child with the given local name.
    pub fn child(&self, name: &str) -> Option<&Element> {
        self.children.iter().find(|c| c.name == name)
    }

    /// Follows a path of child names, e.g. `["Person", "Vorname"]`.
    pub fn path(&self, path: &[&str]) -> Option<&Element> {
        path.iter().try_fold(self, |e, name| e.child(name))
    }

    /// Text at `path`, `None` if missing or empty.
    pub fn text_at(&self, path: &[&str]) -> Option<String> {
        self.path(path)
            .map(|e| e.text.clone())
            .filter(|t| !t.is_empty())
    }

    /// Depth-first search for an element whose child path matches `path`;
    /// the first segment may be any descendant (or `self`).
    pub fn find(&self, path: &[&str]) -> Option<&Element> {
        let (first, rest) = path.split_first()?;
        if self.name == *first
            && let Some(hit) = self.path(rest)
        {
            return Some(hit);
        }
        self.children.iter().find_map(|c| c.find(path))
    }

    /// Text of [`Element::find`], `None` if missing or empty.
    pub fn find_text(&self, path: &[&str]) -> Option<String> {
        self.find(path)
            .map(|e| e.text.clone())
            .filter(|t| !t.is_empty())
    }

    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// All leaves as `(path, text)` pairs, depth first.
    pub fn leaves(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        self.collect_leaves(String::new(), &mut out);
        out
    }

    fn collect_leaves(&self, prefix: String, out: &mut Vec<(String, String)>) {
        let path = if prefix.is_empty() {
            self.name.clone()
        } else {
            format!("{prefix}/{}", self.name)
        };
        if self.children.is_empty() {
            out.push((path, self.text.clone()));
        } else {
            for c in &self.children {
                c.collect_leaves(path.clone(), out);
            }
        }
    }
}

/// Decompresses a gzip member. Trailing bytes after the member are ignored.
pub fn gunzip(data: &[u8], file: &'static str) -> Result<Vec<u8>, EgkError> {
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(data)
        .take(MAX_XML_LEN + 1)
        .read_to_end(&mut out)
        .map_err(|_| EgkError::Format {
            file,
            reason: "invalid gzip data",
        })?;
    if out.len() as u64 > MAX_XML_LEN {
        return Err(EgkError::Format {
            file,
            reason: "decompressed document too large",
        });
    }
    Ok(out)
}

/// Decodes ISO-8859-15 (Latin-9) to UTF-8.
pub fn decode_latin9(bytes: &[u8]) -> String {
    let (text, _) = encoding_rs::ISO_8859_15.decode_without_bom_handling(bytes);
    text.into_owned()
}

/// Parses an XML document into an element tree, ignoring namespaces.
pub fn parse_xml(xml: &str, file: &'static str) -> Result<Element, EgkError> {
    let bad = |reason| EgkError::Format { file, reason };
    let mut reader = Reader::from_str(xml);
    let mut stack: Vec<Element> = Vec::new();
    let mut root = None;

    loop {
        match reader.read_event().map_err(|_| bad("malformed XML"))? {
            Event::Start(e) => stack.push(start_element(&e, file)?),
            Event::Empty(e) => {
                let el = start_element(&e, file)?;
                close(&mut stack, &mut root, el, file)?;
            }
            Event::End(_) => {
                let mut el = stack.pop().ok_or(bad("unbalanced end tag"))?;
                el.text = el.text.trim().to_owned();
                close(&mut stack, &mut root, el, file)?;
            }
            Event::Text(t) => {
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&t.xml10_content());
                }
            }
            Event::CData(t) => {
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&t.xml10_content());
                }
            }
            Event::GeneralRef(r) => {
                let resolved = match r
                    .resolve_char_ref()
                    .map_err(|_| bad("bad character reference"))?
                {
                    Some(c) => c.to_string(),
                    None => {
                        let name: &str = &r;
                        match name {
                            "amp" => "&",
                            "lt" => "<",
                            "gt" => ">",
                            "quot" => "\"",
                            "apos" => "'",
                            _ => return Err(bad("unknown entity")),
                        }
                        .to_owned()
                    }
                };
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&resolved);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !stack.is_empty() {
        return Err(bad("unclosed element"));
    }
    root.ok_or(bad("no root element"))
}

fn start_element(
    e: &quick_xml::events::BytesStart<'_>,
    file: &'static str,
) -> Result<Element, EgkError> {
    let bad = |reason| EgkError::Format { file, reason };
    let name = e.local_name().as_ref().to_owned();
    let mut attributes = Vec::new();
    for a in e.attributes() {
        let a = a.map_err(|_| bad("bad attribute"))?;
        let key = a.key.local_name().as_ref().to_owned();
        if a.key.as_ref().starts_with("xmlns") {
            continue;
        }
        let value = a
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .map_err(|_| bad("bad attribute value"))?
            .into_owned();
        attributes.push((key, value));
    }
    Ok(Element {
        name,
        attributes,
        ..Element::default()
    })
}

fn close(
    stack: &mut [Element],
    root: &mut Option<Element>,
    el: Element,
    file: &'static str,
) -> Result<(), EgkError> {
    match stack.last_mut() {
        Some(parent) => parent.children.push(el),
        None if root.is_none() => *root = Some(el),
        None => {
            return Err(EgkError::Format {
                file,
                reason: "more than one root element",
            });
        }
    }
    Ok(())
}

/// gunzip → ISO-8859-15 → XML tree.
pub fn parse_gzip_xml(data: &[u8], file: &'static str) -> Result<Element, EgkError> {
    let raw = gunzip(data, file)?;
    parse_xml(&decode_latin9(&raw), file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespaces_entities_and_empty_elements() {
        let xml = r#"<?xml version="1.0" encoding="ISO-8859-15"?>
            <vsd:Root xmlns:vsd="urn:x" CDM_VERSION="5.2.0">
              <vsd:A> x &amp; y &#228; </vsd:A>
              <B/>
              <C><D>1</D><D>2</D></C>
            </vsd:Root>"#;
        let root = parse_xml(xml, "test").unwrap();
        assert_eq!(root.name, "Root");
        assert_eq!(root.attribute("CDM_VERSION"), Some("5.2.0"));
        assert_eq!(root.text_at(&["A"]).unwrap(), "x & y ä");
        assert!(root.child("B").is_some());
        assert_eq!(root.text_at(&["B"]), None);
        assert_eq!(root.path(&["C"]).unwrap().children.len(), 2);
        assert_eq!(
            root.leaves(),
            [
                ("Root/A".to_owned(), "x & y ä".to_owned()),
                ("Root/B".to_owned(), String::new()),
                ("Root/C/D".to_owned(), "1".to_owned()),
                ("Root/C/D".to_owned(), "2".to_owned()),
            ]
        );
    }

    #[test]
    fn latin9_euro_sign() {
        // 0xA4 is the euro sign in ISO-8859-15 (currency sign in ISO-8859-1).
        assert_eq!(
            decode_latin9(&[0x4D, 0xFC, 0x6C, 0x6C, 0x65, 0x72, 0x20, 0xA4]),
            "Müller €"
        );
    }

    #[test]
    fn malformed() {
        assert!(parse_xml("<a><b></a>", "t").is_err());
        assert!(parse_xml("<a/><b/>", "t").is_err());
        assert!(parse_xml("", "t").is_err());
        assert!(gunzip(&[0x1F, 0x8B, 0x00], "t").is_err());
    }
}

//! Translations with Project Fluent.
//!
//! Every `crates/kye-app/i18n/<code>.ftl` file is a language (embedded by
//! `build.rs`); English (`en.ftl`) is the source, the default and the fallback
//! for messages a translation does not have yet.

use std::sync::OnceLock;

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource, FluentValue};
use unic_langid::{LanguageIdentifier, langid};

include!(concat!(env!("OUT_DIR"), "/languages.rs"));

/// Code of the source and fallback language.
const SOURCE: &str = "en";

/// A bundled UI language (index into [`LANGUAGES`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lang(usize);

/// `language-name` of each language, in [`LANGUAGES`] order.
fn native_names() -> &'static [String] {
    static NAMES: OnceLock<Vec<String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        LANGUAGES
            .iter()
            .enumerate()
            .map(|(i, (code, _))| {
                format(&bundle(Lang(i)), "language-name", None)
                    .unwrap_or_else(|| (*code).to_owned())
            })
            .collect()
    })
}

/// Normalises `de_DE.UTF-8` / `pt-br` to `de-de` / `pt-br` for comparisons.
fn normalize(code: &str) -> String {
    code.split(['.', '@'])
        .next()
        .unwrap_or_default()
        .replace('_', "-")
        .to_ascii_lowercase()
}

impl Lang {
    pub fn english() -> Self {
        Self::from_code(SOURCE).unwrap_or(Lang(0))
    }

    /// Exact match on the file name, `pt_BR` and `pt-BR` being the same.
    pub fn from_code(code: &str) -> Option<Self> {
        let wanted = normalize(code);
        LANGUAGES
            .iter()
            .position(|(c, _)| normalize(c) == wanted)
            .map(Lang)
    }

    /// All languages, English first, then by their own name.
    pub fn all() -> Vec<Lang> {
        let mut all: Vec<Lang> = (0..LANGUAGES.len()).map(Lang).collect();
        let english = Self::english();
        all.sort_by_key(|l| (*l != english, l.native_name().to_lowercase()));
        all
    }

    pub fn code(self) -> &'static str {
        LANGUAGES[self.0].0
    }

    /// Name of the language in that language (`language-name`), or its code.
    pub fn native_name(self) -> &'static str {
        native_names()
            .get(self.0)
            .map_or(self.code(), String::as_str)
    }

    fn source(self) -> &'static str {
        LANGUAGES[self.0].1
    }

    fn langid(self) -> LanguageIdentifier {
        self.code()
            .replace('_', "-")
            .parse()
            .unwrap_or(langid!("en"))
    }

    /// Maps a locale such as `de-DE` or `pt_BR.UTF-8` to the closest bundled
    /// language: exact match, then same primary language, else English.
    pub fn from_locale(locale: &str) -> Self {
        let locale = normalize(locale);
        if let Some(l) = Self::from_code(&locale) {
            return l;
        }
        let primary = locale.split('-').next().unwrap_or_default();
        Self::from_code(primary)
            .or_else(|| {
                LANGUAGES
                    .iter()
                    .position(|(c, _)| normalize(c).split('-').next() == Some(primary))
                    .map(Lang)
            })
            .unwrap_or_else(Self::english)
    }

    /// Language of the operating system, English if unknown.
    pub fn detect() -> Self {
        sys_locale::get_locale()
            .map(|l| Self::from_locale(&l))
            .unwrap_or_else(Self::english)
    }
}

fn bundle(lang: Lang) -> FluentBundle<FluentResource> {
    let resource = match FluentResource::try_new(lang.source().to_owned()) {
        Ok(r) => r,
        Err((partial, errors)) => {
            tracing::error!(
                lang = lang.code(),
                ?errors,
                "translation file has syntax errors"
            );
            partial
        }
    };
    let mut bundle = FluentBundle::new(vec![lang.langid()]);
    // egui does not render Unicode bidi isolation marks around placeables.
    bundle.set_use_isolating(false);
    if let Err(errors) = bundle.add_resource(resource) {
        tracing::error!(
            lang = lang.code(),
            ?errors,
            "duplicate translation messages"
        );
    }
    bundle
}

/// Message lookup for the current language with English fallback.
pub struct I18n {
    lang: Lang,
    primary: FluentBundle<FluentResource>,
    fallback: FluentBundle<FluentResource>,
}

impl I18n {
    pub fn new(lang: Lang) -> Self {
        Self {
            lang,
            primary: bundle(lang),
            fallback: bundle(Lang::english()),
        }
    }

    pub fn lang(&self) -> Lang {
        self.lang
    }

    /// Translates message `id`.
    pub fn t(&self, id: &str) -> String {
        self.t_with(id, &[])
    }

    /// Translates message `id` with named arguments (`{ $name }` in Fluent).
    pub fn t_with(&self, id: &str, args: &[(&str, FluentValue<'_>)]) -> String {
        let mut fluent_args = FluentArgs::new();
        for (k, v) in args {
            fluent_args.set(*k, v.clone());
        }
        let args = (!args.is_empty()).then_some(&fluent_args);
        format(&self.primary, id, args)
            .or_else(|| format(&self.fallback, id, args))
            .unwrap_or_else(|| {
                tracing::warn!(id, "missing translation");
                id.to_owned()
            })
    }
}

fn format(
    bundle: &FluentBundle<FluentResource>,
    id: &str,
    args: Option<&FluentArgs<'_>>,
) -> Option<String> {
    let pattern = bundle.get_message(id)?.value()?;
    let mut errors = Vec::new();
    let text = bundle.format_pattern(pattern, args, &mut errors);
    if !errors.is_empty() {
        tracing::warn!(id, ?errors, "translation formatting errors");
    }
    Some(text.into_owned())
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use fluent_syntax::ast;
    use fluent_syntax::parser;

    use super::*;

    /// Messages of a file and the variables (`$name`) each one uses.
    fn messages(code: &str, source: &str) -> BTreeMap<String, BTreeSet<String>> {
        let resource = parser::parse(source)
            .unwrap_or_else(|(_, e)| panic!("{code}.ftl has syntax errors: {e:?}"));
        resource
            .body
            .iter()
            .filter_map(|e| match e {
                ast::Entry::Message(m) => {
                    let mut vars = BTreeSet::new();
                    if let Some(p) = &m.value {
                        pattern_vars(p, &mut vars);
                    }
                    Some((m.id.name.to_owned(), vars))
                }
                _ => None,
            })
            .collect()
    }

    fn pattern_vars(p: &ast::Pattern<&str>, out: &mut BTreeSet<String>) {
        for el in &p.elements {
            if let ast::PatternElement::Placeable { expression } = el {
                expression_vars(expression, out);
            }
        }
    }

    fn expression_vars(e: &ast::Expression<&str>, out: &mut BTreeSet<String>) {
        match e {
            ast::Expression::Inline(i) => inline_vars(i, out),
            ast::Expression::Select { selector, variants } => {
                inline_vars(selector, out);
                for v in variants {
                    pattern_vars(&v.value, out);
                }
            }
        }
    }

    fn inline_vars(i: &ast::InlineExpression<&str>, out: &mut BTreeSet<String>) {
        match i {
            ast::InlineExpression::VariableReference { id } => {
                out.insert(id.name.to_owned());
            }
            ast::InlineExpression::Placeable { expression } => expression_vars(expression, out),
            _ => {}
        }
    }

    fn english() -> BTreeMap<String, BTreeSet<String>> {
        messages(SOURCE, Lang::english().source())
    }

    /// String literals in the UI code that look like message ids
    /// (`lower-case-with-dashes`): arguments of `t()`/`t_with()` and the
    /// ids chosen in `match` arms.
    fn used_ids() -> BTreeSet<String> {
        let src = include_str!("app.rs");
        src.split('"')
            .skip(1)
            .step_by(2)
            .filter(|lit| {
                lit.contains('-')
                    && lit.bytes().all(|b| b.is_ascii_lowercase() || b == b'-')
                    && !lit.starts_with('-')
                    && !lit.ends_with('-')
            })
            .map(str::to_owned)
            .chain(["language-name".to_owned()])
            .collect()
    }

    #[test]
    fn english_is_bundled() {
        assert_eq!(Lang::english().code(), "en");
        assert_eq!(Lang::all()[0], Lang::english());
        assert!(Lang::from_code("de").is_some());
    }

    #[test]
    fn every_used_message_exists_in_english() {
        let en = english();
        assert!(used_ids().len() > 40, "scanner found too few ids");
        let missing: Vec<_> = used_ids()
            .into_iter()
            .filter(|id| !en.contains_key(id))
            .collect();
        assert!(missing.is_empty(), "not in en.ftl: {missing:?}");
    }

    #[test]
    fn no_unused_messages_in_english() {
        let used = used_ids();
        let unused: Vec<_> = english()
            .into_keys()
            .filter(|id| !used.contains(id))
            .collect();
        assert!(unused.is_empty(), "unused in en.ftl: {unused:?}");
    }

    /// Translations may be incomplete (English fills the gaps), but every
    /// message they do have must exist in English and use the same variables.
    #[test]
    fn translations_are_valid() {
        let en = english();
        for (code, source) in LANGUAGES {
            let messages = messages(code, source);
            for (id, vars) in &messages {
                let expected = en
                    .get(id)
                    .unwrap_or_else(|| panic!("{code}.ftl: `{id}` does not exist in en.ftl"));
                assert_eq!(
                    vars, expected,
                    "{code}.ftl: `{id}` must use the variables {expected:?}"
                );
            }
            let missing = en.keys().filter(|id| !messages.contains_key(*id)).count();
            if missing > 0 {
                println!("{code}.ftl: {missing} messages not translated yet (English is used)");
            }
        }
    }

    #[test]
    fn locale_detection() {
        let de = Lang::from_code("de").unwrap();
        assert_eq!(Lang::from_locale("de-DE"), de);
        assert_eq!(Lang::from_locale("de_AT.UTF-8"), de);
        assert_eq!(Lang::from_locale("DE"), de);
        assert_eq!(Lang::from_locale("en-US"), Lang::english());
        assert_eq!(Lang::from_locale("xx-YY"), Lang::english());
        assert_eq!(Lang::from_locale(""), Lang::english());
        assert_eq!(de.native_name(), "Deutsch");
        assert_eq!(Lang::english().native_name(), "English");
    }

    #[test]
    fn arguments_and_fallback() {
        let de = I18n::new(Lang::from_code("de").unwrap());
        let text = de.t_with("error-card-text", &[("detail", "X".into())]);
        assert!(text.contains('X') && !text.contains('\u{2068}'), "{text}");
        assert_eq!(de.t("no-such-message"), "no-such-message");
        let en = I18n::new(Lang::english());
        let date = [
            ("year", "2024".into()),
            ("month", "01".into()),
            ("day", "31".into()),
        ];
        assert_eq!(en.t_with("date-format", &date), "2024-01-31");
        assert_eq!(de.t_with("date-format", &date), "31.01.2024");
    }
}

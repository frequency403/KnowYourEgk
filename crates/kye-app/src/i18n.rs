//! Translations with Project Fluent. One `.ftl` file per language lives in
//! `crates/kye-app/i18n/`; English is the default and the fallback for
//! missing messages.

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource, FluentValue};
use unic_langid::{LanguageIdentifier, langid};

const EN_FTL: &str = include_str!("../i18n/en.ftl");
const DE_FTL: &str = include_str!("../i18n/de.ftl");

/// Supported UI languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    De,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::En, Lang::De];

    /// Name of the language in that language (not translated on purpose).
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::De => "Deutsch",
        }
    }

    fn langid(self) -> LanguageIdentifier {
        match self {
            Lang::En => langid!("en"),
            Lang::De => langid!("de"),
        }
    }

    fn source(self) -> &'static str {
        match self {
            Lang::En => EN_FTL,
            Lang::De => DE_FTL,
        }
    }

    /// Maps a BCP 47 locale such as `de-DE` to a supported language,
    /// defaulting to English.
    pub fn from_locale(locale: &str) -> Self {
        let primary = locale
            .split(['-', '_', '.'])
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        match primary.as_str() {
            "de" => Lang::De,
            _ => Lang::En,
        }
    }

    /// Language of the operating system, English if unknown.
    pub fn detect() -> Self {
        sys_locale::get_locale()
            .map(|l| Self::from_locale(&l))
            .unwrap_or(Lang::En)
    }
}

fn bundle(lang: Lang) -> FluentBundle<FluentResource> {
    let resource = match FluentResource::try_new(lang.source().to_owned()) {
        Ok(r) => r,
        Err((partial, errors)) => {
            tracing::error!(?lang, ?errors, "translation file has syntax errors");
            partial
        }
    };
    let mut bundle = FluentBundle::new(vec![lang.langid()]);
    // egui does not render Unicode bidi isolation marks around placeables.
    bundle.set_use_isolating(false);
    if let Err(errors) = bundle.add_resource(resource) {
        tracing::error!(?lang, ?errors, "duplicate translation messages");
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
            fallback: bundle(Lang::En),
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
    use std::collections::BTreeSet;

    use fluent_bundle::FluentResource;
    use fluent_syntax::ast;

    use super::*;

    fn message_ids(source: &str) -> BTreeSet<String> {
        let resource = FluentResource::try_new(source.to_owned())
            .unwrap_or_else(|(_, e)| panic!("syntax errors: {e:?}"));
        resource
            .entries()
            .filter_map(|e| match e {
                ast::Entry::Message(m) => Some(m.id.name.to_owned()),
                _ => None,
            })
            .collect()
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
            .collect()
    }

    #[test]
    fn all_languages_have_the_same_messages() {
        let en = message_ids(EN_FTL);
        for lang in Lang::ALL {
            let other = message_ids(lang.source());
            let missing: Vec<_> = en.difference(&other).collect();
            let extra: Vec<_> = other.difference(&en).collect();
            assert!(missing.is_empty(), "{lang:?} is missing {missing:?}");
            assert!(extra.is_empty(), "{lang:?} has unknown {extra:?}");
        }
    }

    #[test]
    fn every_used_message_exists() {
        let en = message_ids(EN_FTL);
        assert!(used_ids().len() > 40, "scanner found too few ids");
        let missing: Vec<_> = used_ids()
            .into_iter()
            .filter(|id| !en.contains(id))
            .collect();
        assert!(missing.is_empty(), "not in en.ftl: {missing:?}");
    }

    #[test]
    fn no_unused_messages() {
        let used = used_ids();
        let unused: Vec<_> = message_ids(EN_FTL)
            .into_iter()
            .filter(|id| !used.contains(id))
            .collect();
        assert!(unused.is_empty(), "unused in en.ftl: {unused:?}");
    }

    #[test]
    fn locale_detection() {
        assert_eq!(Lang::from_locale("de-DE"), Lang::De);
        assert_eq!(Lang::from_locale("de_AT.UTF-8"), Lang::De);
        assert_eq!(Lang::from_locale("en-US"), Lang::En);
        assert_eq!(Lang::from_locale("fr-FR"), Lang::En);
        assert_eq!(Lang::from_locale(""), Lang::En);
    }

    #[test]
    fn arguments_and_fallback() {
        let de = I18n::new(Lang::De);
        let text = de.t_with("error-card", &[("detail", "X".into())]);
        assert!(text.contains('X') && !text.contains('\u{2068}'), "{text}");
        assert_eq!(de.t("no-such-message"), "no-such-message");
    }
}

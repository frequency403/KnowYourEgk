# Translating KnowYourEgk

Thank you for helping! KnowYourEgk is a small desktop app that shows what is
stored on a German electronic health card (eGK). Users are ordinary patients,
not IT experts: keep the language **plain, calm and friendly**. Many texts
explain privacy or security – please translate those **accurately**, never
stronger or weaker than the English source.

## Two ways to contribute

1. **On Weblate (no programming needed):** translate in the browser on
   [Weblate](https://hosted.weblate.org/projects/knowyouregk/). Weblate sends the result to this repository as a
   pull request.
2. **On GitHub:** copy `crates/kye-app/i18n/en.ftl` to
   `crates/kye-app/i18n/<code>.ftl` (for example `fr.ftl`, `pt_BR.ftl`),
   translate the values and open a pull request. No code change is needed:
   every `.ftl` file in that folder becomes a language in the app.

Partial translations are welcome. Messages that are missing in a language
are shown in English.

## How the texts work

- The source language is **English** (`en.ftl`).
- The files use [Fluent](https://projectfluent.org/fluent/guide/) syntax:
  `message-id = Text`. Only translate the text after `=`; never change the
  message id.
- `language-name` is the name of your language **in your language** (for
  example `Français`). It is shown in the language menu.
- **Placeholders** like `{ $detail }`, `{ $path }`, `{ $code }` or
  `{ $version }` are filled in by the app. Keep them exactly as they are
  (including the `$` and the spaces inside the braces). You may move them
  within the sentence. A test checks that every translation uses the same
  placeholders as English.
- **`date-format`** only defines the date layout, e.g.
  `{ $day }.{ $month }.{ $year }` or `{ $month }/{ $day }/{ $year }`. Use the
  usual numeric date format of your language; do not add words.
- Keep typographic characters where they fit your language: the ellipsis `…`
  in progress texts (e.g. “Reading …”) and the middle dot `·` used as a
  separator. Use the quotation marks of your language (“…”, „…“, « … »).

## Keep it short

Button labels, tabs, reader status and the language/appearance selectors have
little space. Aim for roughly the length of the English text.

## Do not translate

| Term | Meaning |
|---|---|
| KnowYourEgk | Name of the app |
| eGK | German electronic health card (you may explain it once, e.g. in `empty-text`) |
| CAN | 6-digit card access number printed on the card. Keep “CAN”; translating the long form is fine |
| PACE | Name of the encryption protocol |
| PC/SC, pcscd, NFC | Technical names |
| EF.PD, EF.VD | File names on the card |
| IK | German institution code of the health insurer |
| WOP, DMP | German billing codes, shown as they are |
| frequency403 | GitHub user name |

## Wording guidelines

- Address the user the way apps in your language usually do (the German
  version uses the informal “du”). Stay consistent.
- “Card reader” is the USB device; “slot” is the contact or contactless part
  of one reader.
- “Contact” = card inserted into the reader; “contactless” = card placed on
  the NFC reader.
- Error texts always say what happened and what to do next; keep both parts.

## Check your translation

```bash
cargo test -p kye-app                          # syntax, unknown ids, placeholders
cargo run -p kye-app                           # pick your language in the header
cargo test -p kye-app preview -- --ignored     # screenshots in target/ui-preview/
```

## Questions

If a source text is unclear or wrong, comment on the string in Weblate or
open an [issue](https://github.com/frequency403/KnowYourEgk/issues/new?template=translation.yml).

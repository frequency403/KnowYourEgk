//! Embeds every `i18n/<code>.ftl` file so that adding a language only needs a
//! new file (for example created by Weblate), no code change.

use std::error::Error;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::{env, fs};

fn main() -> Result<(), Box<dyn Error>> {
    let dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?).join("i18n");
    println!("cargo:rerun-if-changed={}", dir.display());

    let mut languages = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("ftl") {
            continue;
        }
        let Some(code) = path.file_stem().and_then(|s| s.to_str()).map(str::to_owned) else {
            continue;
        };
        let valid = !code.is_empty()
            && code
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !valid {
            println!(
                "cargo:warning=skipping translation file with invalid name: {}",
                path.display()
            );
            continue;
        }
        println!("cargo:rerun-if-changed={}", path.display());
        languages.push((code, path));
    }
    languages.sort();
    if !languages.iter().any(|(code, _)| code == "en") {
        return Err("i18n/en.ftl (the source language) is missing".into());
    }

    let mut out = String::from(
        "/// `(language code, Fluent source)` of every bundled translation, sorted by code.\n\
         pub const LANGUAGES: &[(&str, &str)] = &[\n",
    );
    for (code, path) in &languages {
        writeln!(
            out,
            "    ({code:?}, include_str!({:?})),",
            path.display().to_string()
        )?;
    }
    out.push_str("];\n");
    fs::write(
        PathBuf::from(env::var("OUT_DIR")?).join("languages.rs"),
        out,
    )?;
    Ok(())
}

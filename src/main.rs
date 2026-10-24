//! `snapshotpb-manual`: builds the stand-alone HTML manual of a board from the firmware
//! document, the application document, and the sources that a manifest names.

mod app;
mod assembly;
mod board;
mod firmware;
mod gen;
mod localize;
mod manifest;
mod markdown;
mod media;
mod names;
mod release;
mod render;
mod schema;
mod site;
mod theme;
mod tree;
mod util;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::Value;

use crate::assembly::AssemblyInfo;
use crate::board::BoardInfo;
use crate::gen::{Ctx, Product};
use crate::manifest::Loaded;
use crate::theme::Theme;
use crate::util::{git_describe, read_bytes, read_text, sha256_short, today_utc};

const USAGE: &str = "usage:
  snapshotpb-manual build <manifest.toml> [--out <file.html>] [--lang <code>] [--source <name>=<path>]...
  snapshotpb-manual check <manifest.toml> [--lang <code>] [--source <name>=<path>]...
  snapshotpb-manual site <manifest.toml>... --out <dir> [--source <name>=<path>]...
  snapshotpb-manual validate <firmware|app> <document.json>...

  build     reads every document and source that the manifest names and writes the manual.
  check     reads every document and source and renders the manual, and writes nothing.
  site      writes one manual for each manifest and each language that its documents hold.
            A manual is at <dir>/<id>/<lang>/index.html. The index page at <dir>/index.html
            lets the reader pick a board and a language.
  validate  checks each document against its schema and its rules, and writes nothing.
  --lang    the language of the manual, for example de. It replaces the language of the manifest.
            A text that a document does not have in that language falls back to English.

  The documents come from the releases that the manifest names, through the gh command.
  gh must be signed in, or GH_TOKEN must hold a token with read access to the releases.
  The downloaded assets live under $SNAPSHOTPB_MANUAL_CACHE, or ~/.cache/snapshotpb-manual.
  --source  replaces a source root of the manifest, for example --source firmware=~/fw.
            The path is relative to the current directory.";

fn main() {
    if let Err(e) = run() {
        eprintln!("snapshotpb-manual: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        bail!("{USAGE}");
    }
    let command = args[0].as_str();
    if command == "validate" {
        return validate(&args[1..]);
    }
    let mut manifests: Vec<PathBuf> = Vec::new();
    let mut out: Option<PathBuf> = None;
    let mut lang: Option<String> = None;
    let mut overrides: Vec<(String, String)> = Vec::new();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out = Some(PathBuf::from(args.get(i).context("--out needs a path")?));
            }
            "--lang" => {
                i += 1;
                lang = Some(args.get(i).context("--lang needs a language code")?.clone());
            }
            "--source" => {
                i += 1;
                let spec = args.get(i).context("--source needs name=path")?;
                let (n, p) = spec.split_once('=').context("--source needs name=path")?;
                overrides.push((n.to_string(), p.to_string()));
            }
            other if other.starts_with("--") => bail!("unknown argument `{other}`\n{USAGE}"),
            path => manifests.push(PathBuf::from(path)),
        }
        i += 1;
    }
    match command {
        "build" | "check" => {
            let [manifest_path] = manifests.as_slice() else {
                bail!("{command} needs one manifest\n{USAGE}");
            };
            if command == "build" {
                build(
                    manifest_path,
                    out.as_deref(),
                    lang.as_deref(),
                    &overrides,
                    true,
                )
            } else {
                build(manifest_path, None, lang.as_deref(), &overrides, false)
            }
        }
        "site" => {
            if manifests.is_empty() {
                bail!("site needs one manifest or more\n{USAGE}");
            }
            if lang.is_some() {
                bail!("site builds each language, and takes no --lang");
            }
            let out = out.context("site needs --out <dir>")?;
            site(&manifests, &out, &overrides)
        }
        other => bail!("unknown command `{other}`\n{USAGE}"),
    }
}

/// The site: each manifest in each of its languages, and the index page.
fn site(manifests: &[PathBuf], out: &Path, overrides: &[(String, String)]) -> Result<()> {
    let mut sections: Vec<site::Section> = Vec::new();
    let mut theme: Option<Theme> = None;
    // The logo of the first manifest is the logo at the head of the index.
    let mut logo: Option<String> = None;
    // The link preview of the first manifest that has one gives the index preview its
    // picture. Each manifest with a preview gives the same address of the site.
    let mut social: Option<manifest::Social> = None;
    for path in manifests {
        let loaded = Loaded::load(path, overrides)?;
        let m = &loaded.manifest;
        // The id is a directory name and a URL segment, so it holds plain characters alone.
        if m.id.is_empty()
            || !m
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            bail!(
                "{}: the id `{}` must hold lower case letters, digits, and hyphens alone",
                path.display(),
                m.id
            );
        }
        if theme.is_none() {
            theme = Some(
                Theme::parse(&loaded.resolve(&m.brand.theme)?)
                    .context("the colour scheme of the app")?,
            );
        }
        if logo.is_none() {
            logo = Some(render::logo(&loaded, site::TITLE, site::LOGO_CLASS)?);
        }
        // Each language reads the documents again through the cache. The work is small,
        // and it keeps `manual` the same for the build command and for the site.
        let languages = languages_of(&loaded)?;
        for lang in &languages {
            let built =
                manual(&loaded, lang).with_context(|| format!("the manual {} in {lang}", m.id))?;
            let page = out.join(&m.id).join(lang).join("index.html");
            write_file(&page, &built.html)?;
            println!("{} ({})", page.display(), built.summary);
        }
        write_file(
            &out.join(&m.id).join("index.html"),
            &site::redirect(
                &format!("{}/", m.language),
                render::manual_preview(m, &m.language).as_ref(),
            ),
        )?;
        // The index has one address, so each manifest of the site must give the same one.
        match (&social, &m.social) {
            (None, Some(s)) => social = Some(s.clone()),
            (Some(first), Some(s)) if first.site_url != s.site_url => bail!(
                "{}: [social] site_url `{}` is not the site_url `{}` of the manifest before it",
                path.display(),
                s.site_url,
                first.site_url
            ),
            _ => {}
        }
        let division = m.brand.division.clone();
        let emblem = match &division {
            Some(name) => render::emblem(&loaded, name, site::EMBLEM_CLASS)?,
            None => None,
        };
        site::place(
            &mut sections,
            division,
            emblem,
            site::Board {
                id: m.id.clone(),
                title: m.title.clone(),
                default_lang: m.language.clone(),
                languages,
            },
        )
        .with_context(|| path.display().to_string())?;
    }
    let theme = theme.context("the site needs one manifest")?;
    let logo = logo.context("the site needs one manifest")?;
    let index = out.join("index.html");
    write_file(
        &index,
        &site::index(&logo, &sections, &theme, social.as_ref()),
    )?;
    let n: usize = sections.iter().map(|s| s.boards.len()).sum();
    println!(
        "{} ({n} {})",
        index.display(),
        if n == 1 { "manual" } else { "manuals" }
    );
    Ok(())
}

/// Each language that the documents of a manifest hold, and the language of the manifest,
/// in sorted order.
fn languages_of(loaded: &Loaded) -> Result<Vec<String>> {
    let m = &loaded.manifest;
    let mut langs = std::collections::BTreeSet::new();
    langs.insert(m.language.clone());
    let mut add = |source: &manifest::DocumentSource, what: &str| -> Result<()> {
        let located = release::locate(loaded, source).context(what.to_string())?;
        let value: Value = serde_json::from_str(&read_text(&located.path)?)
            .with_context(|| format!("{} is not JSON", located.path.display()))?;
        langs.extend(localize::languages(&value));
        Ok(())
    };
    if let Some(f) = &m.firmware {
        add(f, "the firmware document")?;
    }
    if let Some(a) = &m.app {
        add(a, "the application document")?;
    }
    Ok(langs.into_iter().collect())
}

fn write_file(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
    }
    std::fs::write(path, text).with_context(|| format!("cannot write {}", path.display()))
}

/// One manual, as the page and the words of its summary line.
struct Built {
    html: String,
    summary: String,
}

fn validate(args: &[String]) -> Result<()> {
    if args.len() < 2 {
        bail!("validate needs a kind and a document\n{USAGE}");
    }
    let kind = match args[0].as_str() {
        "firmware" => schema::Kind::Firmware,
        "app" => schema::Kind::App,
        other => bail!("unknown document kind `{other}`: use firmware or app"),
    };
    for path in &args[1..] {
        let p = Path::new(path);
        match kind {
            schema::Kind::Firmware => {
                let doc = firmware::Document::load(p, localize::FALLBACK)?;
                println!(
                    "ok: {} (schema {}, {} {}, {} screens, {} settings, {} topics)",
                    p.display(),
                    doc.schema_version,
                    doc.firmware.name,
                    doc.firmware.version,
                    doc.screens.len(),
                    doc.settings.len(),
                    doc.topics.len()
                );
            }
            schema::Kind::App => {
                let doc = app::Document::load(p, localize::FALLBACK)?;
                println!(
                    "ok: {} (schema {}, {} {}, {} targets, {} settings, {} topics)",
                    p.display(),
                    doc.schema_version,
                    doc.app.name,
                    doc.app.version,
                    doc.targets.len(),
                    doc.settings.len(),
                    doc.topics.len()
                );
            }
        }
    }
    Ok(())
}

fn build(
    manifest_path: &Path,
    out: Option<&Path>,
    lang: Option<&str>,
    overrides: &[(String, String)],
    write: bool,
) -> Result<()> {
    let loaded = Loaded::load(manifest_path, overrides)?;
    let m = &loaded.manifest;
    let lang = lang.unwrap_or(&m.language).to_string();
    let built = manual(&loaded, &lang)?;
    let out_path = match out {
        Some(p) => p.to_path_buf(),
        None => loaded.resolve(&m.output)?,
    };
    if write {
        write_file(&out_path, &built.html)?;
        println!("{} ({})", out_path.display(), built.summary);
    } else {
        println!("ok: {}", built.summary);
    }
    Ok(())
}

/// The manual of one manifest in one language.
fn manual(loaded: &Loaded, lang: &str) -> Result<Built> {
    let m = &loaded.manifest;
    let lang = lang.to_string();
    if !localize::is_language_tag(&lang) {
        bail!("the language `{lang}` is not a language tag such as en, de, or es-MX");
    }

    let theme =
        Theme::parse(&loaded.resolve(&m.brand.theme)?).context("the colour scheme of the app")?;
    let fw_source = m
        .firmware
        .as_ref()
        .map(|f| release::locate(loaded, f))
        .transpose()
        .context("the firmware document")?;
    let fw = fw_source
        .as_ref()
        .map(|s| firmware::Document::load(&s.path, &lang))
        .transpose()
        .context("the firmware document")?;
    let fw_dir = fw_source
        .as_ref()
        .and_then(|s| s.path.parent())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let (app_doc, app_source) = match &m.app {
        Some(a) => {
            let source = release::locate(loaded, a).context("the application document")?;
            (
                Some(app::Document::load(&source.path, &lang).context("the application document")?),
                Some(source),
            )
        }
        None => (None, None),
    };
    let app_path = app_source.as_ref().map(|s| s.path.clone());
    let app_dir = app_path
        .as_ref()
        .and_then(|p| p.parent())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    // The manifest check makes sure that an app document comes with a firmware document.
    if let (Some(a), Some(fw)) = (&app_doc, &fw) {
        a.check_against(|k| fw.settings.contains_key(k))
            .context("the application document")?;
        if let (Some(fp), Some(ap)) = (&fw.firmware.protocol_version, &a.app.protocol_version) {
            if fp != ap {
                bail!(
                    "the firmware document speaks protocol {fp}, and the application document speaks protocol {ap}"
                );
            }
        }
    }
    let board = m
        .board
        .as_ref()
        .map(|b| -> Result<BoardInfo> {
            BoardInfo::load(&loaded.resolve(&b.metrics)?).context("the board metrics")
        })
        .transpose()?;
    let assembly = m
        .assembly
        .as_ref()
        .map(|a| -> Result<AssemblyInfo> {
            AssemblyInfo::load(&loaded.resolve(&a.metrics)?).context("the assembly metrics")
        })
        .transpose()?;
    let placed = match &fw {
        Some(fw) => fw.walk()?,
        None => Vec::new(),
    };

    // The values that the content can print.
    let mut values: BTreeMap<String, String> = BTreeMap::new();
    values.insert("id".into(), m.id.clone());
    values.insert("title".into(), m.title.clone());
    values.insert("subtitle".into(), m.subtitle.clone());
    if let (Some(b), Some(board)) = (&m.board, &board) {
        values.insert("board.name".into(), b.name.clone());
        values.insert("board.md5".into(), board.md5_short().to_string());
        values.insert("board.kicad".into(), board.kicad.clone());
        values.insert("board.measured_on".into(), board.measured_on.clone());
        values.insert("board.width_mm".into(), format!("{:.2}", board.width_mm));
        values.insert("board.height_mm".into(), format!("{:.2}", board.height_mm));
    }
    if let (Some(a), Some(assembly)) = (&m.assembly, &assembly) {
        values.insert("assembly.name".into(), a.name.clone());
        values.insert("assembly.md5".into(), assembly.md5_short().to_string());
        values.insert("assembly.measured_on".into(), assembly.measured_on.clone());
        values.insert(
            "assembly.length_mm".into(),
            format!("{:.2}", assembly.length_mm),
        );
        values.insert(
            "assembly.diameter_mm".into(),
            format!("{:.2}", assembly.diameter_mm),
        );
    }
    if let Some(fw) = &fw {
        values.insert("firmware.name".into(), fw.firmware.name.clone());
        values.insert("firmware.version".into(), fw.firmware.version.clone());
        if let Some(s) = &fw.firmware.summary {
            values.insert("firmware.summary".into(), s.clone());
        }
        if let Some(p) = &fw.firmware.protocol_version {
            values.insert("firmware.protocol_version".into(), p.clone());
        }
    }
    if let Some(a) = &app_doc {
        values.insert("app.name".into(), a.app.name.clone());
        values.insert("app.version".into(), a.app.version.clone());
        if let Some(s) = &a.app.summary {
            values.insert("app.summary".into(), s.clone());
        }
    }
    let date = today_utc();
    // The year of the copyright line: the first four digits of the date.
    values.insert("manual.year".into(), date.chars().take(4).collect());
    values.insert("manual.date".into(), date);
    values.insert("manual.language".into(), lang.clone());
    for (k, v) in &m.values {
        values.insert(k.clone(), v.clone());
    }

    let mut provenance: Vec<(String, String, String)> = Vec::new();
    if let (Some(fw), Some(s)) = (&fw, &fw_source) {
        provenance.push((
            "Firmware document".into(),
            s.place.clone(),
            format!(
                "{} {}, sha256 {}",
                fw.firmware.name,
                fw.firmware.version,
                sha256_short(&read_bytes(&s.path)?)
            ),
        ));
    }
    if let (Some(a), Some(s)) = (&app_doc, &app_source) {
        let p = &s.path;
        provenance.push((
            "Application document".into(),
            s.place.clone(),
            format!(
                "{} {}, sha256 {}",
                a.app.name,
                a.app.version,
                sha256_short(&read_bytes(p)?)
            ),
        ));
    }
    if let Some(fw) = &fw {
        provenance.push((
            "Schema set".into(),
            "schemas/".into(),
            format!(
                "firmware {} (document {}), app {}{}",
                schema_version(schema::FIRMWARE),
                fw.schema_version,
                schema_version(schema::APP),
                app_doc
                    .as_ref()
                    .map(|a| format!(" (document {})", a.schema_version))
                    .unwrap_or_default()
            ),
        ));
    }
    for (name, root) in loaded.sources() {
        provenance.push((
            format!("{name} checkout"),
            root.display().to_string(),
            git_describe(root),
        ));
    }
    if let Some(board) = &board {
        provenance.push((
            "Board".into(),
            board.file.clone(),
            format!(
                "md5 {} at commit {}, KiCad {}, measured on {}",
                board.md5_short(),
                board.measured_at,
                board.kicad,
                board.measured_on
            ),
        ));
    }
    if let Some(assembly) = &assembly {
        provenance.push((
            "Assembly".into(),
            assembly.file.clone(),
            format!(
                "md5 {} at commit {}, measured on {}",
                assembly.md5_short(),
                assembly.measured_at,
                assembly.measured_on
            ),
        ));
    }
    provenance.push((
        "Colour scheme".into(),
        m.brand.theme.clone(),
        format!(
            "sha256 {}",
            sha256_short(read_text(&loaded.resolve(&m.brand.theme)?)?.as_bytes())
        ),
    ));
    provenance.push((
        "Manual".into(),
        loaded.path.display().to_string(),
        format!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")),
    ));

    // The manifest check makes sure that the manifest names a board or an assembly.
    let product = match (&m.board, &board, &m.assembly, &assembly) {
        (Some(b), Some(info), None, None) => Product::Board {
            info,
            name: &b.name,
        },
        (None, None, Some(a), Some(info)) => Product::Assembly {
            info,
            name: &a.name,
        },
        _ => bail!("the manifest must name a [board] or an [assembly]"),
    };
    let ctx = Ctx {
        lang,
        loaded,
        fw: fw.as_ref(),
        fw_dir: &fw_dir,
        app: app_doc.as_ref(),
        app_dir: &app_dir,
        product,
        placed,
        values,
        provenance,
    };
    let content_dir = loaded.resolve(&m.content)?;
    let body = markdown::render_dir(&content_dir, &ctx).context("the chapters")?;
    let html = render::page(&ctx, &theme, &body)?;
    let mut summary = format!("{} bytes", html.len());
    if let Some(fw) = &fw {
        summary.push_str(&format!(
            ", {} screens, {} settings",
            fw.screens.len(),
            fw.settings.len()
        ));
    }
    summary.push_str(&format!(", {} contents entries", body.toc.len()));
    if let Some(fw) = &fw {
        summary.push_str(&format!(", firmware {}", fw.firmware.version));
    }
    if let Some(a) = &app_doc {
        summary.push_str(&format!(", app {}", a.app.version));
    }
    if let Some(a) = &assembly {
        summary.push_str(&format!(", assembly {}", a.md5_short()));
    }
    Ok(Built { html, summary })
}

/// The `const` of `schema_version` in a schema of the set.
fn schema_version(schema_text: &str) -> String {
    serde_json::from_str::<Value>(schema_text)
        .ok()
        .and_then(|v| {
            v["properties"]["schema_version"]["const"]
                .as_str()
                .map(str::to_string)
        })
        .unwrap_or_else(|| "unknown".to_string())
}

//! `snapshotpb-manual`: builds the stand-alone HTML manual of a board from the firmware
//! document, the application document, and the sources that a manifest names.

mod app;
mod board;
mod firmware;
mod gen;
mod manifest;
mod markdown;
mod media;
mod names;
mod render;
mod schema;
mod theme;
mod tree;
mod util;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::Value;

use crate::board::BoardInfo;
use crate::gen::Ctx;
use crate::manifest::Loaded;
use crate::theme::Theme;
use crate::util::{git_describe, read_bytes, read_text, sha256_short, today_utc};

const USAGE: &str = "usage:
  snapshotpb-manual build <manifest.toml> [--out <file.html>] [--source <name>=<path>]...
  snapshotpb-manual check <manifest.toml> [--source <name>=<path>]...
  snapshotpb-manual validate <firmware|app> <document.json>...

  build     reads every document and source that the manifest names and writes the manual.
  check     reads every document and source and renders the manual, and writes nothing.
  validate  checks each document against its schema and its rules, and writes nothing.
  --source  replaces a source root of the manifest, for example --source firmware=~/fw";

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
    let manifest_path = PathBuf::from(&args[1]);
    let mut out: Option<PathBuf> = None;
    let mut overrides: Vec<(String, String)> = Vec::new();
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out = Some(PathBuf::from(args.get(i).context("--out needs a path")?));
            }
            "--source" => {
                i += 1;
                let spec = args.get(i).context("--source needs name=path")?;
                let (n, p) = spec.split_once('=').context("--source needs name=path")?;
                overrides.push((n.to_string(), p.to_string()));
            }
            other => bail!("unknown argument `{other}`\n{USAGE}"),
        }
        i += 1;
    }
    match command {
        "build" => build(&manifest_path, out.as_deref(), &overrides, true),
        "check" => build(&manifest_path, None, &overrides, false),
        other => bail!("unknown command `{other}`\n{USAGE}"),
    }
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
                let doc = firmware::Document::load(p)?;
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
                let doc = app::Document::load(p)?;
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
    overrides: &[(String, String)],
    write: bool,
) -> Result<()> {
    let loaded = Loaded::load(manifest_path, overrides)?;
    let m = &loaded.manifest;

    let theme =
        Theme::parse(&loaded.resolve(&m.brand.theme)?).context("the colour scheme of the app")?;
    let fw_path = loaded.resolve(&m.firmware.document)?;
    let fw = firmware::Document::load(&fw_path).context("the firmware document")?;
    let fw_dir = fw_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let (app_doc, app_path) = match &m.app {
        Some(a) => {
            let p = loaded.resolve(&a.document)?;
            (
                Some(app::Document::load(&p).context("the application document")?),
                Some(p),
            )
        }
        None => (None, None),
    };
    let app_dir = app_path
        .as_ref()
        .and_then(|p| p.parent())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    if let Some(a) = &app_doc {
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
    let board = BoardInfo::load(&loaded.resolve(&m.board.metrics)?).context("the board metrics")?;
    let placed = fw.walk()?;

    // The values that the content can print.
    let mut values: BTreeMap<String, String> = BTreeMap::new();
    values.insert("id".into(), m.id.clone());
    values.insert("title".into(), m.title.clone());
    values.insert("subtitle".into(), m.subtitle.clone());
    values.insert("board.name".into(), m.board.name.clone());
    values.insert("board.md5".into(), board.md5_short().to_string());
    values.insert("board.kicad".into(), board.kicad.clone());
    values.insert("board.measured_on".into(), board.measured_on.clone());
    values.insert("board.width_mm".into(), format!("{:.2}", board.width_mm));
    values.insert("board.height_mm".into(), format!("{:.2}", board.height_mm));
    values.insert("firmware.name".into(), fw.firmware.name.clone());
    values.insert("firmware.version".into(), fw.firmware.version.clone());
    if let Some(s) = &fw.firmware.summary {
        values.insert("firmware.summary".into(), s.clone());
    }
    if let Some(p) = &fw.firmware.protocol_version {
        values.insert("firmware.protocol_version".into(), p.clone());
    }
    if let Some(a) = &app_doc {
        values.insert("app.name".into(), a.app.name.clone());
        values.insert("app.version".into(), a.app.version.clone());
        if let Some(s) = &a.app.summary {
            values.insert("app.summary".into(), s.clone());
        }
    }
    values.insert("manual.date".into(), today_utc());
    for (k, v) in &m.values {
        values.insert(k.clone(), v.clone());
    }

    let mut provenance: Vec<(String, String, String)> = Vec::new();
    provenance.push((
        "Firmware document".into(),
        m.firmware.document.clone(),
        format!(
            "{} {}, sha256 {}",
            fw.firmware.name,
            fw.firmware.version,
            sha256_short(&read_bytes(&fw_path)?)
        ),
    ));
    if let (Some(a), Some(p), Some(spec)) = (&app_doc, &app_path, &m.app) {
        provenance.push((
            "Application document".into(),
            spec.document.clone(),
            format!(
                "{} {}, sha256 {}",
                a.app.name,
                a.app.version,
                sha256_short(&read_bytes(p)?)
            ),
        ));
    }
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
    for (name, root) in loaded.sources() {
        provenance.push((
            format!("{name} checkout"),
            root.display().to_string(),
            git_describe(root),
        ));
    }
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

    let ctx = Ctx {
        loaded: &loaded,
        fw: &fw,
        fw_dir: &fw_dir,
        app: app_doc.as_ref(),
        app_dir: &app_dir,
        board: &board,
        placed,
        values,
        provenance,
    };
    let content_dir = loaded.resolve(&m.content)?;
    let body = markdown::render_dir(&content_dir, &ctx).context("the chapters")?;
    let html = render::page(&ctx, &theme, &body)?;

    let out_path = match out {
        Some(p) => p.to_path_buf(),
        None => loaded.resolve(&m.output)?,
    };
    let summary = format!(
        "{} bytes, {} screens, {} settings, {} contents entries, firmware {}{}",
        html.len(),
        fw.screens.len(),
        fw.settings.len(),
        body.toc.len(),
        fw.firmware.version,
        app_doc
            .as_ref()
            .map(|a| format!(", app {}", a.app.version))
            .unwrap_or_default()
    );
    if write {
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("cannot create {}", parent.display()))?;
        }
        std::fs::write(&out_path, &html)
            .with_context(|| format!("cannot write {}", out_path.display()))?;
        println!("{} ({summary})", out_path.display());
    } else {
        println!("ok: {summary}");
    }
    Ok(())
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

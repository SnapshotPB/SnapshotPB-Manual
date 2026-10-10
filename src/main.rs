//! `snapshotpb-manual`: builds the stand-alone HTML manual of a board and its firmware from
//! the sources that a manifest names.

mod board;
mod defaults;
mod fsm;
mod gen;
mod manifest;
mod markdown;
mod menu;
mod proto;
mod render;
mod theme;
mod util;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::board::BoardInfo;
use crate::defaults::Defaults;
use crate::fsm::Fsm;
use crate::gen::{Ctx, Descriptions};
use crate::manifest::Loaded;
use crate::menu::{Lang, MenuModel};
use crate::proto::ProtoModel;
use crate::theme::Theme;
use crate::util::{git_describe, read_bytes, read_text, sha256_short, today_utc};

const USAGE: &str = "usage:
  snapshotpb-manual build <manifest.toml> [--out <file.html>] [--source <name>=<path>]...
  snapshotpb-manual check <manifest.toml> [--source <name>=<path>]...

  build  reads every source that the manifest names and writes the manual.
  check  reads every source and renders the manual, and writes nothing.
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
    let menu =
        MenuModel::load(&loaded.firmware_path(&m.firmware.menu_tree)?).context("the menu tree")?;
    let lang =
        Lang::load(&loaded.firmware_path(&m.firmware.menu_lang)?).context("the menu words")?;
    let fsm = Fsm::load(&loaded.firmware_path(&m.firmware.interaction_fsm)?)
        .context("the interaction FSM")?;
    let proto_root = loaded.firmware_path(&m.firmware.proto_root)?;
    let proto = ProtoModel::load(&proto_root).context("the protocol")?;
    let mut defaults = Defaults::default();
    for k in &m.firmware.kconfig {
        defaults.add_kconfig(&loaded.firmware_path(k)?)?;
    }
    for d in &m.firmware.defines {
        defaults.add_defines(&loaded.firmware_path(d)?)?;
    }
    let board = BoardInfo::load(&loaded.resolve(&m.board.metrics)?).context("the board metrics")?;

    // Every bind must resolve: a row that binds a field that the protocol lost is an error.
    for s in &menu.screens {
        for it in &s.items {
            if let Some(b) = &it.bind {
                let (scope, field) = b
                    .split_once('.')
                    .with_context(|| format!("bind `{b}` has no scope"))?;
                if scope == "runtime" {
                    continue;
                }
                let msg = match m.firmware.scopes.get(scope) {
                    Some(msg) => msg,
                    None => bail!("the row `{}` binds the scope `{scope}`, and the manifest maps no message to it (firmware.scopes)", it.render),
                };
                if proto.field(msg, field).is_none() {
                    bail!(
                        "the row `{}` binds `{b}`, and the message `{msg}` has no field `{field}`",
                        it.render
                    );
                }
            }
        }
    }

    let content_dir = loaded.resolve(&m.content)?;
    let descriptions: Descriptions =
        match std::fs::read_to_string(content_dir.join("settings.toml")) {
            Ok(t) => toml::from_str(&t).context("content/settings.toml")?,
            Err(_) => Descriptions::default(),
        };

    // The values that the content can print.
    let firmware_root = loaded.source("firmware")?.to_path_buf();
    let fw_version = git_describe(&firmware_root);
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
    values.insert("firmware.version".into(), fw_version.clone());
    values.insert("firmware.board".into(), m.firmware.board.clone());
    values.insert("manual.date".into(), today_utc());
    for (k, v) in &m.values {
        values.insert(k.clone(), v.clone());
    }

    let mut provenance: Vec<(String, String, String)> = Vec::new();
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
        "Protocol".into(),
        proto_root.display().to_string(),
        git_describe(&proto_root),
    ));
    for (what, spec) in [
        ("Menu tree", &m.firmware.menu_tree),
        ("Menu words", &m.firmware.menu_lang),
        ("Interaction FSM", &m.firmware.interaction_fsm),
    ] {
        let p = loaded.firmware_path(spec)?;
        provenance.push((
            what.into(),
            spec.clone(),
            format!("sha256 {}", sha256_short(&read_bytes(&p)?)),
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

    let ctx = Ctx {
        loaded: &loaded,
        menu: &menu,
        lang: &lang,
        proto: &proto,
        fsm: &fsm,
        defaults: &defaults,
        board: &board,
        descriptions: &descriptions,
        values,
        provenance,
    };
    let body = markdown::render_dir(&content_dir, &ctx).context("the chapters")?;
    let html = render::page(&ctx, &theme, &body)?;

    let out_path = match out {
        Some(p) => p.to_path_buf(),
        None => loaded.resolve(&m.output)?,
    };
    if write {
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("cannot create {}", parent.display()))?;
        }
        std::fs::write(&out_path, &html)
            .with_context(|| format!("cannot write {}", out_path.display()))?;
        println!(
            "{} ({} bytes, {} screens, {} contents entries, firmware {})",
            out_path.display(),
            html.len(),
            menu.screens.len(),
            body.toc.len(),
            fw_version
        );
    } else {
        println!(
            "ok: {} bytes, {} screens, {} contents entries, firmware {}",
            html.len(),
            menu.screens.len(),
            body.toc.len(),
            fw_version
        );
    }
    Ok(())
}

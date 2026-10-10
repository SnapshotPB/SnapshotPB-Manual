//! The manifest of one manual: where its sources are, and how to read them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::util::{expand_tilde, read_text};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// A short identifier. It names the output file when `output` does not.
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub subtitle: String,
    /// The HTML file that the build writes. A `name:` prefix selects a source root.
    pub output: String,
    /// The directory of the Markdown chapters.
    pub content: String,
    /// Named source roots. A path `name:rest` in this file resolves against `sources.name`.
    #[serde(default)]
    pub sources: BTreeMap<String, String>,
    pub brand: Brand,
    pub board: Board,
    pub firmware: Firmware,
    /// Where the default of a setting comes from, by its `bind` key.
    #[serde(default)]
    pub defaults: BTreeMap<String, String>,
    /// Literal values for `{{value name}}` in the content.
    #[serde(default)]
    pub values: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Brand {
    pub logo: String,
    pub icon: String,
    /// The Kotlin file that holds the colour scheme of the app.
    pub theme: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Board {
    pub name: String,
    /// The `METRICS.json` of the board.
    pub metrics: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Firmware {
    /// The Zephyr board target.
    pub board: String,
    pub menu_tree: String,
    pub menu_lang: String,
    pub interaction_fsm: String,
    pub proto_root: String,
    #[serde(default)]
    pub kconfig: Vec<String>,
    #[serde(default)]
    pub defines: Vec<String>,
    /// A `bind` scope and the protocol message that holds it.
    #[serde(default)]
    pub scopes: BTreeMap<String, String>,
    /// An input event of the menu and the words for it.
    #[serde(default)]
    pub events: BTreeMap<String, String>,
    /// A state of the menu and the words for it, in the order of the manual.
    #[serde(default)]
    pub states: Vec<StateWords>,
    /// A guard or a predicate and the words for it.
    #[serde(default)]
    pub guards: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateWords {
    pub state: String,
    pub name: String,
    #[serde(default)]
    pub text: String,
}

/// A manifest with its directory and the source roots that the command line changed.
pub struct Loaded {
    pub manifest: Manifest,
    pub dir: PathBuf,
    pub path: PathBuf,
    sources: BTreeMap<String, PathBuf>,
}

impl Loaded {
    pub fn load(path: &Path, overrides: &[(String, String)]) -> Result<Loaded> {
        let text = read_text(path)?;
        let manifest: Manifest =
            toml::from_str(&text).with_context(|| format!("manifest {}", path.display()))?;
        let dir = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let mut sources = BTreeMap::new();
        for (name, spec) in &manifest.sources {
            sources.insert(name.clone(), Self::plain(&dir, spec));
        }
        for (name, spec) in overrides {
            sources.insert(name.clone(), Self::plain(&dir, spec));
        }
        Ok(Loaded {
            manifest,
            dir,
            path: path.to_path_buf(),
            sources,
        })
    }

    fn plain(dir: &Path, spec: &str) -> PathBuf {
        let p = expand_tilde(spec);
        if p.is_absolute() {
            p
        } else {
            dir.join(p)
        }
    }

    /// The directory of a named source root.
    pub fn source(&self, name: &str) -> Result<&Path> {
        match self.sources.get(name) {
            Some(p) => Ok(p),
            None => bail!("the manifest names no source `{name}`"),
        }
    }

    /// Resolves a path of the manifest or of the content: `name:rest`, `~/…`, absolute, or
    /// relative to the manifest.
    pub fn resolve(&self, spec: &str) -> Result<PathBuf> {
        if let Some((name, rest)) = spec.split_once(':') {
            if !name.is_empty() && !name.contains('/') && !name.contains('\\') {
                if let Some(root) = self.sources.get(name) {
                    return Ok(root.join(rest));
                }
                bail!("`{spec}` names the source `{name}`, and the manifest has no such source");
            }
        }
        Ok(Self::plain(&self.dir, spec))
    }

    /// Resolves a path that the firmware section gives relative to the firmware root.
    pub fn firmware_path(&self, spec: &str) -> Result<PathBuf> {
        if spec.contains(':') || spec.starts_with('~') || Path::new(spec).is_absolute() {
            return self.resolve(spec);
        }
        Ok(self.source("firmware")?.join(spec))
    }

    pub fn sources(&self) -> &BTreeMap<String, PathBuf> {
        &self.sources
    }
}

//! The manifest of one manual: which documents it renders, and where its sources are.

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
    /// Named source roots. A path `name:rest` in this file, in a chapter, or in the `src` of
    /// a media item resolves against `sources.name`.
    #[serde(default)]
    pub sources: BTreeMap<String, String>,
    pub brand: Brand,
    pub board: Board,
    pub firmware: Firmware,
    #[serde(default)]
    pub app: Option<App>,
    /// The language of the manual, as a language tag such as `en` or `de`. A text that a
    /// document does not have in this language falls back to English.
    #[serde(default = "default_language")]
    pub language: String,
    /// Literal values for `{{value name}}` in the content.
    #[serde(default)]
    pub values: BTreeMap<String, String>,
    /// The readable name of each designator that the page shows by name, such as `J11`.
    #[serde(default)]
    pub names: BTreeMap<String, String>,
}

fn default_language() -> String {
    crate::localize::FALLBACK.to_string()
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
    /// The firmware document, which follows `schemas/firmware/v2`.
    pub document: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct App {
    /// The application document, which follows `schemas/app/v2`.
    pub document: String,
}

/// A manifest with its directory and the source roots that the command line changed.
pub struct Loaded {
    pub manifest: Manifest,
    pub dir: PathBuf,
    pub path: PathBuf,
    sources: BTreeMap<String, PathBuf>,
}

impl Loaded {
    /// Reads a manifest, and applies the source roots that the command line replaced.
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

    /// A manifest with no source, for a test.
    #[cfg(test)]
    pub fn empty(path: &Path) -> Loaded {
        Loaded {
            manifest: Manifest {
                id: String::new(),
                title: String::new(),
                subtitle: String::new(),
                output: String::new(),
                content: String::new(),
                sources: BTreeMap::new(),
                brand: Brand {
                    logo: String::new(),
                    icon: String::new(),
                    theme: String::new(),
                },
                board: Board {
                    name: String::new(),
                    metrics: String::new(),
                },
                firmware: Firmware {
                    document: String::new(),
                },
                app: None,
                values: BTreeMap::new(),
                names: BTreeMap::new(),
                language: default_language(),
            },
            dir: path.parent().map(Path::to_path_buf).unwrap_or_default(),
            path: path.to_path_buf(),
            sources: BTreeMap::new(),
        }
    }

    fn plain(dir: &Path, spec: &str) -> PathBuf {
        let p = expand_tilde(spec);
        if p.is_absolute() {
            p
        } else {
            dir.join(p)
        }
    }

    /// Resolves a path of the manifest, of the content, or of a media item: `name:rest`,
    /// `~/…`, absolute, or relative to the manifest.
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

    pub fn sources(&self) -> &BTreeMap<String, PathBuf> {
        &self.sources
    }
}

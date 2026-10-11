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
    /// The firmware document, which follows `schemas/firmware/v2`.
    pub firmware: DocumentSource,
    /// The application document, which follows `schemas/app/v2`. Absent for a manual of
    /// the board alone.
    #[serde(default)]
    pub app: Option<DocumentSource>,
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
    /// The emblem of the board family. It sits beside the logo at the head of the manual.
    #[serde(default)]
    pub emblem: Option<String>,
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

/// Where a document comes from: a file, or an asset of a GitHub release. A source names
/// one of the two.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentSource {
    /// The path of the document. A `name:` prefix selects a source root.
    pub document: Option<String>,
    /// The release that holds the document, as `owner/repository@tag`.
    pub release: Option<String>,
    /// The name of the asset of the release.
    pub asset: Option<String>,
    /// The SHA-256 of the asset, as 64 hex digits, when the manifest pins it. The asset is
    /// verified against the digest of the release in each case.
    pub sha256: Option<String>,
}

impl DocumentSource {
    /// The rule that the schema of the manifest cannot state: a file or a release, not both
    /// and not neither.
    fn check(&self, owner: &str) -> Result<()> {
        match (&self.document, &self.release, &self.asset) {
            (Some(_), None, None) => {
                if self.sha256.is_some() {
                    bail!("[{owner}] pins a sha256 for a document file; the pin is for an asset of a release");
                }
                Ok(())
            }
            (None, Some(_), Some(_)) => {
                if let Some(pin) = &self.sha256 {
                    if pin.len() != 64 || !pin.chars().all(|c| c.is_ascii_hexdigit()) {
                        bail!("[{owner}] sha256 is not 64 hex digits");
                    }
                }
                Ok(())
            }
            (None, Some(_), None) | (None, None, Some(_)) => {
                bail!("[{owner}] needs both `release` and `asset`")
            }
            (None, None, None) => {
                bail!("[{owner}] needs a `document`, or a `release` with an `asset`")
            }
            (Some(_), _, _) => {
                bail!("[{owner}] names a `document` and a `release` or an `asset`; name a `document`, or a `release` with an `asset`")
            }
        }
    }
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
        manifest
            .firmware
            .check("firmware")
            .with_context(|| format!("manifest {}", path.display()))?;
        if let Some(app) = &manifest.app {
            app.check("app")
                .with_context(|| format!("manifest {}", path.display()))?;
        }
        let dir = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let mut sources = BTreeMap::new();
        for (name, spec) in &manifest.sources {
            sources.insert(name.clone(), Self::plain(&dir, spec));
        }
        // A path on the command line is relative to the current directory, as the user
        // typed it. A path in the manifest is relative to the manifest.
        for (name, spec) in overrides {
            sources.insert(name.clone(), expand_tilde(spec));
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
                    emblem: None,
                    icon: String::new(),
                    theme: String::new(),
                },
                board: Board {
                    name: String::new(),
                    metrics: String::new(),
                },
                firmware: DocumentSource {
                    document: Some(String::new()),
                    release: None,
                    asset: None,
                    sha256: None,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn source(
        document: Option<&str>,
        release: Option<&str>,
        asset: Option<&str>,
        sha256: Option<&str>,
    ) -> DocumentSource {
        DocumentSource {
            document: document.map(String::from),
            release: release.map(String::from),
            asset: asset.map(String::from),
            sha256: sha256.map(String::from),
        }
    }

    #[test]
    fn a_source_is_a_file_or_a_release_with_an_asset() {
        let hex = "3cef1d571052db1ef70914ea03e09758e858f68684f7f9822f2dda8debd1c68f";
        assert!(source(Some("a.json"), None, None, None)
            .check("firmware")
            .is_ok());
        assert!(source(None, Some("o/r@v1"), Some("a.json"), None)
            .check("firmware")
            .is_ok());
        assert!(source(None, Some("o/r@v1"), Some("a.json"), Some(hex))
            .check("firmware")
            .is_ok());
        let err = |s: DocumentSource| s.check("app").unwrap_err().to_string();
        assert!(err(source(None, Some("o/r@v1"), None, None)).contains("both"));
        assert!(err(source(None, None, Some("a.json"), None)).contains("both"));
        assert!(err(source(None, None, None, None)).contains("needs a"));
        assert!(err(source(Some("a.json"), Some("o/r@v1"), None, None)).contains("name a"));
        assert!(err(source(Some("a.json"), None, Some("a.json"), None)).contains("name a"));
        assert!(err(source(Some("a.json"), None, None, Some(hex))).contains("pins"));
        assert!(err(source(None, Some("o/r@v1"), Some("a.json"), Some("abc"))).contains("64 hex"));
    }

    #[test]
    fn a_source_override_is_relative_to_the_current_directory() {
        let dir =
            std::env::temp_dir().join(format!("snapshotpb-manual-override-{}", std::process::id()));
        let manifests = dir.join("manifests");
        std::fs::create_dir_all(&manifests).expect("a temporary directory");
        let path = manifests.join("m.toml");
        std::fs::write(
            &path,
            r#"id = "m"
title = "M"
output = "m.html"
content = "c"
[sources]
pcb = "../pcb"
[brand]
logo = "l.svg"
icon = "i.svg"
theme = "t.kt"
[board]
name = "B"
metrics = "m.json"
[firmware]
document = "f.json"
"#,
        )
        .expect("a manifest");
        let loaded = Loaded::load(&path, &[("app".to_string(), "sources/app".to_string())]);
        std::fs::remove_dir_all(&dir).expect("the temporary directory goes");
        let loaded = loaded.expect("the manifest loads");
        assert_eq!(loaded.sources()["app"], PathBuf::from("sources/app"));
        assert_eq!(loaded.sources()["pcb"], manifests.join("../pcb"));
    }
}

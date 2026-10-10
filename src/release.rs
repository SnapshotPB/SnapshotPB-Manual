//! A document from a GitHub release. The firmware repository and the app repository attach
//! the document of each release to that release, so the manual of a firmware and an app
//! comes from the documents of their releases and from nothing else.
//!
//! The manifest names the release as `owner/repository@tag` and the asset by name. The
//! module reads the release through the `gh` command, which carries the sign-in for a
//! private repository, downloads the asset into a cache, and verifies its SHA-256 against
//! the digest that GitHub holds for the asset, and against the digest that the manifest
//! pins when it pins one. A cached asset that verifies is used with no download.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde_json::Value;

use crate::manifest::{DocumentSource, Loaded};
use crate::util::{read_bytes, sha256_hex};

/// The environment variable that names the cache directory of the downloaded assets.
const CACHE_ENV: &str = "SNAPSHOTPB_MANUAL_CACHE";

/// A document on disk, and the words that say where it came from.
pub struct Located {
    pub path: PathBuf,
    /// For the provenance table: the manifest path, or the release and the asset.
    pub place: String,
}

/// The file of a document source: the path that the manifest names, or the asset of the
/// release that it names, downloaded and verified.
pub fn locate(loaded: &Loaded, source: &DocumentSource) -> Result<Located> {
    if let Some(doc) = &source.document {
        return Ok(Located {
            path: loaded.resolve(doc)?,
            place: doc.clone(),
        });
    }
    let (Some(release), Some(asset)) = (&source.release, &source.asset) else {
        bail!("the source names no document and no release");
    };
    let (repository, tag) = parse_release(release)?;
    let path = fetch(
        &repository,
        &tag,
        asset,
        source.sha256.as_deref(),
        &cache_dir(),
    )?;
    Ok(Located {
        path,
        place: format!("{repository} release {tag}, asset {asset}"),
    })
}

/// `owner/repository@tag` as its two parts. Each part is a plain name, so the three parts
/// make a safe path under the cache directory.
fn parse_release(spec: &str) -> Result<(String, String)> {
    let Some((repository, tag)) = spec.split_once('@') else {
        bail!("the release `{spec}` is not owner/repository@tag");
    };
    let parts: Vec<&str> = repository.split('/').collect();
    if parts.len() != 2 || !parts.iter().all(|p| plain_name(p)) || !plain_name(tag) {
        bail!("the release `{spec}` is not owner/repository@tag");
    }
    Ok((repository.to_string(), tag.to_string()))
}

/// True for a name that can stand as one step of a path: letters, digits, `-`, `_` and
/// `.`, and not `.` or `..` alone.
fn plain_name(s: &str) -> bool {
    !s.is_empty()
        && s != "."
        && s != ".."
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
}

/// The hex digest of a `sha256:<hex>` digest of the GitHub API.
fn parse_digest(digest: &str) -> Result<String> {
    match digest.strip_prefix("sha256:") {
        Some(hex) if hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit()) => {
            Ok(hex.to_ascii_lowercase())
        }
        _ => bail!("the digest `{digest}` is not sha256:<64 hex digits>"),
    }
}

/// The SHA-256 of the bytes, when it equals the digest of the release and the digest that
/// the manifest pins.
fn verify(bytes: &[u8], release_digest: &str, pinned: Option<&str>, what: &str) -> Result<String> {
    let actual = sha256_hex(bytes);
    if actual != release_digest {
        bail!("{what} has the SHA-256 {actual}, and the release says {release_digest}");
    }
    if let Some(pin) = pinned {
        if actual != pin.to_ascii_lowercase() {
            bail!("{what} has the SHA-256 {actual}, and the manifest pins {pin}");
        }
    }
    Ok(actual)
}

/// The directory of the downloaded assets: `$SNAPSHOTPB_MANUAL_CACHE`, or the cache
/// directory of the user, or `.cache` under the working directory.
fn cache_dir() -> PathBuf {
    // An empty variable counts as no variable, as the XDG specification says.
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty());
    if let Some(dir) = var(CACHE_ENV) {
        return PathBuf::from(dir);
    }
    if let Some(dir) = var("XDG_CACHE_HOME") {
        return PathBuf::from(dir).join("snapshotpb-manual");
    }
    if let Some(home) = var("HOME") {
        return PathBuf::from(home).join(".cache").join("snapshotpb-manual");
    }
    PathBuf::from(".cache").join("snapshotpb-manual")
}

/// The asset of a release on disk, verified. A cached copy that verifies is used; otherwise
/// the asset is downloaded.
fn fetch(
    repository: &str,
    tag: &str,
    asset: &str,
    pinned: Option<&str>,
    cache: &Path,
) -> Result<PathBuf> {
    let what = format!("the asset {asset} of {repository} release {tag}");
    if asset.contains(['/', '\\']) || asset == "." || asset == ".." {
        bail!("the asset name `{asset}` is not a file name");
    }
    let release = gh_json(&["api", &format!("repos/{repository}/releases/tags/{tag}")])
        .with_context(|| format!("cannot read {repository} release {tag}"))?;
    let entry = release
        .get("assets")
        .and_then(Value::as_array)
        .and_then(|a| {
            a.iter()
                .find(|e| e.get("name").and_then(Value::as_str) == Some(asset))
        });
    let Some(entry) = entry else {
        bail!("{repository} release {tag} has no asset named {asset}");
    };
    let digest = entry
        .get("digest")
        .and_then(Value::as_str)
        .with_context(|| format!("GitHub holds no digest for {what}"))?;
    let digest = parse_digest(digest)?;
    if let Some(pin) = pinned {
        if pin.to_ascii_lowercase() != digest {
            bail!("the manifest pins {pin} for {what}, and the release says {digest}");
        }
    }
    let url = entry
        .get("url")
        .and_then(Value::as_str)
        .with_context(|| format!("GitHub gives no URL for {what}"))?;

    let path = cache.join(repository).join(tag).join(asset);
    if path.is_file() {
        if let Ok(bytes) = read_bytes(&path) {
            if verify(&bytes, &digest, pinned, &what).is_ok() {
                return Ok(path);
            }
        }
    }
    let bytes = gh_bytes(&["api", url, "-H", "Accept: application/octet-stream"])
        .with_context(|| format!("cannot download {what}"))?;
    verify(&bytes, &digest, pinned, &what)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("cannot make {}", dir.display()))?;
    }
    // A write to a temporary name and a rename, so that another build that runs at the same
    // time never reads a partial file.
    let partial = path.with_extension(format!("part{}", std::process::id()));
    std::fs::write(&partial, &bytes)
        .with_context(|| format!("cannot write {}", partial.display()))?;
    std::fs::rename(&partial, &path)
        .with_context(|| format!("cannot move {} into place", partial.display()))?;
    Ok(path)
}

fn gh_bytes(args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("gh").args(args).output().context(
        "cannot run the gh command. The documents come from private releases: install gh and sign in, or set GH_TOKEN",
    )?;
    if !output.status.success() {
        bail!(
            "gh {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(output.stdout)
}

fn gh_json(args: &[&str]) -> Result<Value> {
    let bytes = gh_bytes(args)?;
    serde_json::from_slice(&bytes).context("the answer of gh is not JSON")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_release_spec_has_a_repository_and_a_tag() {
        assert_eq!(
            parse_release("SnapshotPB/Firmware@v2026.41.1").unwrap(),
            ("SnapshotPB/Firmware".to_string(), "v2026.41.1".to_string())
        );
        for bad in [
            "Firmware@v1",
            "SnapshotPB/Firmware",
            "a/b@",
            "a/b/c@v1",
            "a/b@v 1",
            "",
            "../b@v1",
            "a/b@..",
            "a/.@v1",
        ] {
            assert!(parse_release(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn a_digest_of_the_api_gives_its_hex() {
        let hex = "3cef1d571052db1ef70914ea03e09758e858f68684f7f9822f2dda8debd1c68f";
        assert_eq!(
            parse_digest(&format!("sha256:{}", hex.to_uppercase())).unwrap(),
            hex
        );
        assert!(parse_digest(hex).is_err());
        assert!(parse_digest("sha256:abc").is_err());
    }

    #[test]
    fn the_bytes_must_match_the_release_and_the_pin() {
        let bytes = b"{}";
        let digest = sha256_hex(bytes);
        assert_eq!(verify(bytes, &digest, None, "x").unwrap(), digest);
        assert_eq!(
            verify(bytes, &digest, Some(&digest.to_uppercase()), "x").unwrap(),
            digest
        );
        let other = sha256_hex(b"{ }");
        assert!(verify(bytes, &other, None, "x")
            .unwrap_err()
            .to_string()
            .contains("the release says"));
        assert!(verify(bytes, &digest, Some(&other), "x")
            .unwrap_err()
            .to_string()
            .contains("the manifest pins"));
    }
}

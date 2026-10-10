//! A media item of a document, and how the page shows it: a picture inline, and each other
//! kind as an embedded file with a link, until the page has a viewer for that kind.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use anyhow::{bail, Context, Result};
use base64::Engine;
use regex::Regex;
use serde::Deserialize;
use serde_json::Value;

use crate::manifest::Loaded;
use crate::util::{esc, read_bytes, read_text};

/// The largest file that the page embeds. A data URI is 4/3 of the file, and a browser
/// stops on a page of some hundred megabytes with no message.
pub const MAX_EMBED_BYTES: u64 = 16 * 1024 * 1024;

/// Reads a file for the page, and refuses one over the limit.
fn read_embed(path: &Path) -> Result<Vec<u8>> {
    let size = std::fs::metadata(path)
        .with_context(|| format!("cannot read {}", path.display()))?
        .len();
    if size > MAX_EMBED_BYTES {
        bail!(
            "{} is {size} bytes, and the page embeds a file of {MAX_EMBED_BYTES} bytes at most",
            path.display()
        );
    }
    read_bytes(path)
}

/// The media type that a file extension implies.
fn media_type_of(ext: &str) -> &'static str {
    match ext {
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "glb" => "model/gltf-binary",
        "gltf" => "model/gltf+json",
        "stl" => "model/stl",
        "step" | "stp" => "model/step",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Media {
    pub kind: String,
    pub src: String,
    pub caption: String,
    pub media_type: Option<String>,
    pub alt: Option<String>,
    pub params: Option<Value>,
}

impl Media {
    /// The file of the item. A plain path is relative to the document; a `source:` prefix
    /// selects a source root of the manifest.
    pub fn path(&self, loaded: &Loaded, document_dir: &Path) -> Result<PathBuf> {
        if let Some((prefix, _)) = self.src.split_once(':') {
            if !prefix.is_empty() && !prefix.contains('/') && !prefix.contains('\\') {
                return loaded.resolve(&self.src);
            }
        }
        Ok(document_dir.join(&self.src))
    }

    /// The media type: the stated one, or the one that the extension implies.
    pub fn media_type(&self, path: &Path) -> String {
        if let Some(t) = &self.media_type {
            return t.clone();
        }
        media_type_of(&extension(path)).to_string()
    }

    /// The figure for the item.
    pub fn html(&self, loaded: &Loaded, document_dir: &Path) -> Result<String> {
        let path = self.path(loaded, document_dir)?;
        let media_type = self.media_type(&path);
        let alt = self.alt.as_deref().unwrap_or(&self.caption);
        let body = match self.kind.as_str() {
            "image" => {
                if media_type == "image/svg+xml" {
                    inline_svg(&path, alt)?
                } else {
                    let data = base64::engine::general_purpose::STANDARD.encode(read_embed(&path)?);
                    format!(
                        "<img src=\"data:{media_type};base64,{data}\" alt=\"{}\" loading=\"lazy\">",
                        esc(alt)
                    )
                }
            }
            "video" => {
                let data = base64::engine::general_purpose::STANDARD.encode(read_embed(&path)?);
                format!(
                    "<video controls preload=\"metadata\" aria-label=\"{}\"><source src=\"data:{media_type};base64,{data}\" type=\"{media_type}\"></video>",
                    esc(alt)
                )
            }
            other => {
                // A kind with no viewer yet: the file travels with the page, and a link opens
                // or saves it.
                let data = base64::engine::general_purpose::STANDARD.encode(read_embed(&path)?);
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
                format!(
                    "<p class=\"media-file\"><a download=\"{}\" href=\"data:{media_type};base64,{data}\">{}</a> <span class=\"kind\">{}</span></p>",
                    esc(name),
                    esc(alt),
                    esc(other)
                )
            }
        };
        // The parameters of the kind travel with the figure, so a viewer for the kind can
        // read them from the page.
        let params = match &self.params {
            Some(p) => format!(" data-params=\"{}\"", esc(&p.to_string())),
            None => String::new(),
        };
        Ok(format!(
            "<figure class=\"media media-{}\"{params}>{body}<figcaption>{}</figcaption></figure>\n",
            esc(&self.kind),
            esc(&self.caption)
        ))
    }
}

/// An SVG file as inline markup: no XML declaration, no comment, and an accessible name.
pub fn inline_svg(path: &Path, alt: &str) -> Result<String> {
    let text = read_text(path)?;
    // The pattern is a constant, so it compiles once and cannot fail.
    static RE_DECL: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?s)<\?xml.*?\?>|<!--.*?-->|<!DOCTYPE[^>]*>").expect("a constant pattern")
    });
    let re_decl = &*RE_DECL;
    let text = re_decl.replace_all(&text, "");
    let start = match text.find("<svg") {
        Some(i) => i,
        None => bail!("{} holds no <svg element", path.display()),
    };
    let text = text[start..].trim().to_string();
    let label = if alt.is_empty() {
        String::new()
    } else {
        format!(" role=\"img\" aria-label=\"{}\"", esc(alt))
    };
    Ok(text.replacen("<svg", &format!("<svg{label}"), 1))
}

/// Embeds an image file by its path, for the `{{image}}` directive of a chapter.
pub fn image_file(path: &Path, alt: &str) -> Result<String> {
    let ext = extension(path);
    let mime = media_type_of(&ext);
    if ext == "svg" {
        return inline_svg(path, alt);
    }
    if !mime.starts_with("image/") {
        bail!(
            "{}: `{ext}` is not an image type that the manual embeds",
            path.display()
        );
    }
    let data = base64::engine::general_purpose::STANDARD
        .encode(read_embed(path).with_context(|| format!("the image {}", path.display()))?);
    Ok(format!(
        "<img src=\"data:{mime};base64,{data}\" alt=\"{}\" loading=\"lazy\">",
        esc(alt)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_media_type_follows_the_extension() {
        let m = Media {
            kind: "model".into(),
            src: "x.glb".into(),
            caption: "c".into(),
            media_type: None,
            alt: None,
            params: None,
        };
        assert_eq!(m.media_type(Path::new("x.glb")), "model/gltf-binary");
        assert_eq!(
            m.media_type(Path::new("x.unknown")),
            "application/octet-stream"
        );
    }

    #[test]
    fn a_plain_path_with_a_colon_stays_under_the_document() {
        let m = Media {
            kind: "image".into(),
            src: "img/a:b.png".into(),
            caption: "c".into(),
            media_type: None,
            alt: None,
            params: None,
        };
        let loaded = crate::manifest::Loaded::empty(Path::new("/m"));
        assert_eq!(
            m.path(&loaded, Path::new("/d")).unwrap(),
            Path::new("/d/img/a:b.png")
        );
    }

    #[test]
    fn a_file_over_the_limit_is_refused() {
        let dir = std::env::temp_dir().join(format!("snapshotpb-manual-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("big.png");
        let f = std::fs::File::create(&p).unwrap();
        f.set_len(MAX_EMBED_BYTES + 1).unwrap();
        let err = format!("{:#}", image_file(&p, "x").unwrap_err());
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(err.contains("at most"), "{err}");
    }

    #[test]
    fn a_stated_media_type_wins() {
        let m = Media {
            kind: "model".into(),
            src: "x.bin".into(),
            caption: "c".into(),
            media_type: Some("model/custom".into()),
            alt: None,
            params: None,
        };
        assert_eq!(m.media_type(Path::new("x.bin")), "model/custom");
    }
}

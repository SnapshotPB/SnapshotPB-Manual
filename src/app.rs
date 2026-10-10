//! The application document: the typed model and the rules that the schema cannot state.

use std::path::Path;

use anyhow::{bail, Context, Result};
use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::Value;

use crate::media::Media;
use crate::schema;
use crate::util::read_text;

/// The target key of the board tab in the manual. An app target cannot use it.
pub const RESERVED_TARGET: &str = "board";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub schema_version: String,
    pub app: App,
    pub targets: IndexMap<String, Target>,
    pub settings: IndexMap<String, Entry>,
    #[serde(default)]
    pub topics: IndexMap<String, Entry>,
    #[serde(default)]
    pub media: IndexMap<String, Media>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct App {
    pub name: String,
    pub version: String,
    pub protocol_version: Option<String>,
    pub summary: Option<String>,
    #[serde(default)]
    pub media: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub title: String,
    pub description: String,
    #[serde(rename = "where")]
    pub place: Option<String>,
    pub editable: Option<bool>,
    #[serde(default)]
    pub media: Vec<String>,
    pub targets: Option<IndexMap<String, Override>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Override {
    pub description: Option<String>,
    #[serde(rename = "where")]
    pub place: Option<String>,
    pub editable: Option<bool>,
    #[serde(default)]
    pub media: Vec<String>,
}

/// The text of one entry for one target, after the override is applied.
pub struct Resolved<'a> {
    pub title: &'a str,
    pub description: &'a str,
    pub place: Option<&'a str>,
    pub editable: bool,
    pub media: &'a [String],
}

impl Document {
    /// Reads a document, validates it against the schema, and checks the rules below.
    pub fn load(path: &Path) -> Result<Document> {
        let text = read_text(path)?;
        let value: Value = serde_json::from_str(&text)
            .with_context(|| format!("{} is not JSON", path.display()))?;
        schema::validate(schema::Kind::App, &value, &path.display().to_string())?;
        let doc: Document = serde_json::from_value(value)
            .with_context(|| format!("{} does not fit the app model", path.display()))?;
        doc.check()
            .with_context(|| format!("{} breaks a rule", path.display()))?;
        Ok(doc)
    }

    /// The rules that JSON Schema cannot state: each target and each media key that an
    /// entry names exists, and no target takes the key of the board tab.
    pub fn check(&self) -> Result<()> {
        if self.targets.contains_key(RESERVED_TARGET) {
            bail!("the target key `{RESERVED_TARGET}` is reserved for the board tab of the manual");
        }
        for (key, entry) in self.settings.iter().chain(self.topics.iter()) {
            self.media_exist(&entry.media, key)?;
            for (t, o) in entry.targets.iter().flatten() {
                if !self.targets.contains_key(t) {
                    bail!("the entry `{key}` names the target `{t}`, and there is no such target");
                }
                self.media_exist(&o.media, key)?;
            }
        }
        self.media_exist(&self.app.media, "the app")?;
        Ok(())
    }

    /// Each setting of the app must be a setting of the firmware, or the app words would
    /// render nowhere. A topic that only the app has is permitted, and the manual renders it
    /// with the tabs of the app alone.
    pub fn check_against(&self, firmware_has_setting: impl Fn(&str) -> bool) -> Result<()> {
        for key in self.settings.keys() {
            if !firmware_has_setting(key) {
                bail!("the app document describes the setting `{key}`, and the firmware document has no such setting");
            }
        }
        Ok(())
    }

    fn media_exist(&self, keys: &[String], owner: &str) -> Result<()> {
        for k in keys {
            if !self.media.contains_key(k) {
                bail!("`{owner}` shows the media `{k}`, and there is no such media item");
            }
        }
        Ok(())
    }

    /// The entry for a shared key, as a setting or as a topic.
    pub fn entry(&self, key: &str) -> Option<&Entry> {
        self.settings.get(key).or_else(|| self.topics.get(key))
    }

    /// The targets that an entry applies to, in the order of the document, with the text
    /// for each one.
    pub fn resolve<'a>(&'a self, entry: &'a Entry) -> Vec<(&'a str, &'a Target, Resolved<'a>)> {
        let mut out = Vec::new();
        for (key, target) in &self.targets {
            let o: Option<&Override> = match &entry.targets {
                Some(map) => match map.get(key) {
                    Some(o) => Some(o),
                    None => continue,
                },
                None => None,
            };
            out.push((
                key.as_str(),
                target,
                Resolved {
                    title: &entry.title,
                    description: o
                        .and_then(|o| o.description.as_deref())
                        .unwrap_or(&entry.description),
                    place: o
                        .and_then(|o| o.place.as_deref())
                        .or(entry.place.as_deref()),
                    editable: o
                        .and_then(|o| o.editable)
                        .or(entry.editable)
                        .unwrap_or(true),
                    media: match o {
                        Some(o) if !o.media.is_empty() => &o.media,
                        _ => &entry.media,
                    },
                },
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc() -> Document {
        serde_json::from_value(serde_json::json!({
            "schema_version": "1.0.0",
            "app": { "name": "App", "version": "1.0.0" },
            "targets": { "android": { "name": "Android" }, "wear": { "name": "Wear OS" } },
            "settings": {
                "a.b": { "title": "B", "description": "All.", "where": "X > Y" },
                "a.c": { "title": "C", "description": "Some.", "targets": { "wear": { "editable": false, "description": "Watch." } } }
            }
        }))
        .unwrap()
    }

    #[test]
    fn an_entry_with_no_targets_applies_to_every_target() {
        let d = doc();
        let r = d.resolve(d.entry("a.b").unwrap());
        let keys: Vec<&str> = r.iter().map(|(k, _, _)| *k).collect();
        assert_eq!(keys, ["android", "wear"]);
        assert!(r[0].2.editable);
        assert_eq!(r[1].2.place, Some("X > Y"));
    }

    #[test]
    fn an_override_limits_and_changes_the_entry() {
        let d = doc();
        let r = d.resolve(d.entry("a.c").unwrap());
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].0, "wear");
        assert!(!r[0].2.editable);
        assert_eq!(r[0].2.description, "Watch.");
    }

    #[test]
    fn a_setting_that_the_firmware_lacks_is_rejected() {
        let d = doc();
        assert!(d.check_against(|k| k == "a.b" || k == "a.c").is_ok());
        let err = d.check_against(|k| k == "a.b").unwrap_err().to_string();
        assert!(err.contains("a.c"), "{err}");
    }

    #[test]
    fn the_board_key_is_reserved() {
        let mut d = doc();
        d.targets.insert(
            "board".into(),
            Target {
                name: "Board".into(),
                description: None,
            },
        );
        assert!(d.check().unwrap_err().to_string().contains("reserved"));
    }

    #[test]
    fn an_unknown_target_is_rejected() {
        let mut d = doc();
        d.settings.get_mut("a.c").unwrap().targets =
            Some(IndexMap::from([("ios".to_string(), Override::default())]));
        assert!(d.check().is_err());
    }
}

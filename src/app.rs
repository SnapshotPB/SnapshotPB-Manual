//! The application document: the typed model and the rules that the schema cannot state.

use std::path::Path;

use anyhow::{bail, Context, Result};
use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::Value;

use crate::localize::localize;
use crate::media::Media;
use crate::schema;
use crate::util::read_text;

/// The tab keys of the manual itself: the board tab and the app tab. An app target cannot
/// use them, because a tab key must select one tab alone.
pub const RESERVED_TARGETS: [&str; 2] = ["board", "app"];

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

/// The words of one entry on one target: the target key, the target, and the resolved text.
pub type Words<'a> = (&'a str, &'a Target, Resolved<'a>);

/// The text of one entry for one target, after the override is applied.
#[derive(Debug, PartialEq, Eq)]
pub struct Resolved<'a> {
    pub title: &'a str,
    pub description: &'a str,
    pub place: Option<&'a str>,
    pub editable: bool,
    pub media: &'a [String],
}

impl Document {
    /// Reads a document, validates it against the schema, selects the text of `lang`, and
    /// checks the rules below.
    pub fn load(path: &Path, lang: &str) -> Result<Document> {
        let text = read_text(path)?;
        let value: Value = serde_json::from_str(&text)
            .with_context(|| format!("{} is not JSON", path.display()))?;
        schema::validate(schema::Kind::App, &value, &path.display().to_string())?;
        let value = localize(value, lang).with_context(|| {
            format!(
                "{} holds a localized text that is not complete",
                path.display()
            )
        })?;
        let doc: Document = serde_json::from_value(value)
            .with_context(|| format!("{} does not fit the app model", path.display()))?;
        doc.check()
            .with_context(|| format!("{} breaks a rule", path.display()))?;
        Ok(doc)
    }

    /// The rules that JSON Schema cannot state: each target and each media key that an
    /// entry names exists, and no target takes a tab key of the manual.
    pub fn check(&self) -> Result<()> {
        for reserved in RESERVED_TARGETS {
            if self.targets.contains_key(reserved) {
                bail!("the target key `{reserved}` is reserved for a tab of the manual");
            }
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
    /// with the app words alone.
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
    pub fn resolve<'a>(&'a self, entry: &'a Entry) -> Vec<Words<'a>> {
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

    /// True when the entry applies to every target of the document.
    pub fn covers_all(&self, entry: &Entry) -> bool {
        match &entry.targets {
            Some(map) => self.targets.keys().all(|k| map.contains_key(k)),
            None => true,
        }
    }
}

/// True when each target gets the same words, so the manual shows them once, with no tab
/// for each platform.
pub fn same_words(resolved: &[Words<'_>]) -> bool {
    resolved.windows(2).all(|w| w[0].2 == w[1].2)
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
    fn the_tab_keys_of_the_manual_are_reserved() {
        for key in RESERVED_TARGETS {
            let mut d = doc();
            d.targets.insert(
                key.into(),
                Target {
                    name: key.into(),
                    description: None,
                },
            );
            assert!(d.check().unwrap_err().to_string().contains("reserved"));
        }
    }

    #[test]
    fn the_same_words_on_each_target_need_no_platform_tabs() {
        let d = doc();
        let all = d.resolve(d.entry("a.b").unwrap());
        assert!(same_words(&all));
        assert!(d.covers_all(d.entry("a.b").unwrap()));

        let mut d = doc();
        let entry = d.settings.get_mut("a.c").unwrap();
        entry.targets = Some(IndexMap::from([
            ("android".to_string(), Override::default()),
            (
                "wear".to_string(),
                Override {
                    description: Some("Watch.".into()),
                    ..Override::default()
                },
            ),
        ]));
        let entry = d.entry("a.c").unwrap();
        let r = d.resolve(entry);
        assert_eq!(r.len(), 2);
        assert!(!same_words(&r));
        assert!(d.covers_all(entry));
    }

    #[test]
    fn an_empty_override_keeps_the_words_and_limits_the_targets() {
        let d = doc();
        let mut entry = Entry {
            title: "B".into(),
            description: "All.".into(),
            place: None,
            editable: None,
            media: vec![],
            targets: Some(IndexMap::from([(
                "android".to_string(),
                Override::default(),
            )])),
        };
        let r = d.resolve(&entry);
        assert_eq!(r.len(), 1);
        assert!(same_words(&r));
        assert!(!d.covers_all(&entry));
        entry.targets = None;
        assert!(d.covers_all(&entry));
    }

    #[test]
    fn an_unknown_target_is_rejected() {
        let mut d = doc();
        d.settings.get_mut("a.c").unwrap().targets =
            Some(IndexMap::from([("ios".to_string(), Override::default())]));
        assert!(d.check().is_err());
    }
}

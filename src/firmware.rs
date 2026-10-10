//! The firmware document: the typed model, the tree rules that the schema cannot state, and
//! the walk of the screens from the root.

use std::path::Path;

use anyhow::{bail, Context, Result};
use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::Value;

use crate::media::Media;
use crate::schema;
use crate::util::read_text;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub schema_version: String,
    pub firmware: Firmware,
    #[serde(default)]
    pub media: IndexMap<String, Media>,
    pub controls: IndexMap<String, Control>,
    pub interactions: IndexMap<String, Interaction>,
    pub home: Home,
    pub root: String,
    pub screens: IndexMap<String, Screen>,
    pub settings: IndexMap<String, Setting>,
    #[serde(default)]
    pub predicates: IndexMap<String, Predicate>,
    #[serde(default)]
    pub topics: IndexMap<String, Topic>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Firmware {
    pub name: String,
    pub version: String,
    pub protocol_version: Option<String>,
    pub board_models: Vec<String>,
    pub summary: Option<String>,
    #[serde(default)]
    pub media: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Control {
    pub name: String,
    pub description: String,
    pub events: IndexMap<String, ControlEvent>,
    #[serde(default)]
    pub media: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlEvent {
    pub description: String,
    pub hold_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Interaction {
    pub control: String,
    pub event: String,
    pub description: String,
    #[serde(default)]
    pub applies_to: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Home {
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub indicators: IndexMap<String, String>,
    #[serde(default)]
    pub media: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Screen {
    pub title: String,
    pub description: Option<String>,
    pub items: Vec<Item>,
    #[serde(default)]
    pub media: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemType {
    Setting,
    Submenu,
    Back,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    #[serde(rename = "type")]
    pub kind: ItemType,
    pub render: String,
    /// The shared key of a setting row.
    pub key: Option<String>,
    /// The screen key of a submenu row.
    #[serde(rename = "ref")]
    pub target: Option<String>,
    pub hidden_when: Option<String>,
    pub disabled_when: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Device,
    Profile,
    Runtime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingType {
    Int,
    Enum,
    Toggle,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Setting {
    pub title: String,
    pub description: String,
    pub scope: Scope,
    #[serde(rename = "type")]
    pub kind: SettingType,
    #[serde(default)]
    pub related: Vec<String>,
    #[serde(default)]
    pub media: Vec<String>,
    pub min: Option<i64>,
    pub max: Option<i64>,
    pub step: Option<i64>,
    pub default: Option<Value>,
    pub unit: Option<String>,
    #[serde(default)]
    pub decimals: u32,
    pub zero_text: Option<String>,
    #[serde(default)]
    pub options: Vec<EnumOption>,
    pub on_text: Option<String>,
    pub off_text: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnumOption {
    pub value: Value,
    pub text: String,
    pub description: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Predicate {
    pub description: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Topic {
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub interactions: Vec<String>,
    #[serde(default)]
    pub related: Vec<String>,
    #[serde(default)]
    pub media: Vec<String>,
}

/// One screen in the walk from the root, with the path of titles above it.
pub struct Placed<'a> {
    pub key: &'a str,
    pub screen: &'a Screen,
    pub depth: usize,
    /// The keys of the screens from the root screen to this screen, this one included.
    pub keys: Vec<&'a str>,
}

impl Document {
    /// Reads a document, validates it against the schema, and checks the tree rules.
    pub fn load(path: &Path) -> Result<Document> {
        let text = read_text(path)?;
        let value: Value = serde_json::from_str(&text)
            .with_context(|| format!("{} is not JSON", path.display()))?;
        schema::validate(schema::Kind::Firmware, &value, &path.display().to_string())?;
        let doc: Document = serde_json::from_value(value)
            .with_context(|| format!("{} does not fit the firmware model", path.display()))?;
        doc.check()
            .with_context(|| format!("{} breaks a tree rule", path.display()))?;
        Ok(doc)
    }

    /// The rules that JSON Schema cannot state. See docs/firmware-schema.md.
    pub fn check(&self) -> Result<()> {
        if !self.screens.contains_key(&self.root) {
            bail!("the root `{}` is not a screen", self.root);
        }
        let mut parents: IndexMap<&str, Vec<&str>> = IndexMap::new();
        for (key, screen) in &self.screens {
            let last = screen
                .items
                .last()
                .with_context(|| format!("the screen `{key}` has no row"))?;
            if last.kind != ItemType::Back {
                bail!("the last row of the screen `{key}` is not a back row");
            }
            for item in &screen.items {
                match item.kind {
                    ItemType::Submenu => {
                        let target = item.target.as_deref().unwrap_or_default();
                        if !self.screens.contains_key(target) {
                            bail!("the row `{}` of `{key}` opens `{target}`, and there is no such screen", item.render);
                        }
                        parents.entry(target).or_default().push(key);
                    }
                    ItemType::Setting => {
                        let k = item.key.as_deref().unwrap_or_default();
                        if !self.settings.contains_key(k) {
                            bail!("the row `{}` of `{key}` names the setting `{k}`, and there is no such setting", item.render);
                        }
                    }
                    ItemType::Back => {}
                }
                for p in [&item.hidden_when, &item.disabled_when]
                    .into_iter()
                    .flatten()
                {
                    if !self.predicates.contains_key(p) {
                        bail!("the row `{}` of `{key}` names the predicate `{p}`, and there is no such predicate", item.render);
                    }
                }
            }
            self.media_exist(&screen.media, &format!("the screen `{key}`"))?;
        }
        for key in self.screens.keys() {
            let n = parents.get(key.as_str()).map_or(0, Vec::len);
            if key == &self.root && n > 0 {
                bail!("the root screen `{key}` is the target of a submenu row");
            }
            if key != &self.root && n != 1 {
                bail!("the screen `{key}` is the target of {n} submenu rows, and a screen needs exactly one");
            }
        }
        let placed = self.walk()?;
        if placed.len() != self.screens.len() {
            let missing: Vec<&String> = self
                .screens
                .keys()
                .filter(|k| !placed.iter().any(|p| p.key == k.as_str()))
                .collect();
            bail!("no path from the root reaches the screens {missing:?}");
        }
        let on_a_screen = |k: &str| {
            self.screens
                .values()
                .flat_map(|s| s.items.iter())
                .any(|i| i.key.as_deref() == Some(k))
        };
        for (key, setting) in &self.settings {
            if !on_a_screen(key) {
                bail!("the setting `{key}` is on no screen");
            }
            for r in &setting.related {
                self.shared_key_exists(r, &format!("the setting `{key}`"))?;
            }
            self.media_exist(&setting.media, &format!("the setting `{key}`"))?;
        }
        for (key, p) in &self.predicates {
            for d in &p.depends_on {
                if !self.settings.contains_key(d) {
                    bail!("the predicate `{key}` depends on `{d}`, and there is no such setting");
                }
            }
        }
        for (key, t) in &self.topics {
            for i in &t.interactions {
                if !self.interactions.contains_key(i) {
                    bail!("the topic `{key}` names the interaction `{i}`, and there is no such interaction");
                }
            }
            for r in &t.related {
                self.shared_key_exists(r, &format!("the topic `{key}`"))?;
            }
            self.media_exist(&t.media, &format!("the topic `{key}`"))?;
        }
        for (key, i) in &self.interactions {
            let control = match self.controls.get(&i.control) {
                Some(c) => c,
                None => bail!(
                    "the interaction `{key}` uses the control `{}`, and there is no such control",
                    i.control
                ),
            };
            if !control.events.contains_key(&i.event) {
                bail!("the interaction `{key}` uses the event `{}` of `{}`, and the control has no such event", i.event, i.control);
            }
        }
        for (key, c) in &self.controls {
            self.media_exist(&c.media, &format!("the control `{key}`"))?;
        }
        self.media_exist(&self.home.media, "the home screen")?;
        self.media_exist(&self.firmware.media, "the firmware")?;
        Ok(())
    }

    fn shared_key_exists(&self, key: &str, owner: &str) -> Result<()> {
        if self.settings.contains_key(key) || self.topics.contains_key(key) {
            return Ok(());
        }
        bail!("{owner} relates to `{key}`, and there is no such setting or topic")
    }

    fn media_exist(&self, keys: &[String], owner: &str) -> Result<()> {
        for k in keys {
            if !self.media.contains_key(k) {
                bail!("{owner} shows the media `{k}`, and there is no such media item");
            }
        }
        Ok(())
    }

    /// The screens in the order of a depth-first walk from the root. The order of the rows
    /// on a screen is the order of its children.
    pub fn walk(&self) -> Result<Vec<Placed<'_>>> {
        let mut out = Vec::new();
        let mut keys = Vec::new();
        self.walk_into(&self.root, &mut keys, &mut out)?;
        Ok(out)
    }

    fn walk_into<'a>(
        &'a self,
        key: &'a str,
        keys: &mut Vec<&'a str>,
        out: &mut Vec<Placed<'a>>,
    ) -> Result<()> {
        let screen = match self.screens.get(key) {
            Some(s) => s,
            None => bail!("a submenu row opens `{key}`, and there is no such screen"),
        };
        if out.iter().any(|p| p.key == key) {
            bail!("the screen `{key}` is its own ancestor");
        }
        keys.push(key);
        out.push(Placed {
            key,
            screen,
            depth: keys.len() - 1,
            keys: keys.clone(),
        });
        for item in &screen.items {
            if item.kind == ItemType::Submenu {
                if let Some(t) = &item.target {
                    self.walk_into(t, keys, out)?;
                }
            }
        }
        keys.pop();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example() -> Document {
        serde_json::from_str(include_str!(
            "../schemas/firmware/v1/examples/autococker-26.29.0.json"
        ))
        .unwrap()
    }

    #[test]
    fn the_example_passes_the_tree_rules() {
        let doc = example();
        doc.check().unwrap();
        let walk = doc.walk().unwrap();
        let keys: Vec<&str> = walk.iter().map(|p| p.key).collect();
        assert_eq!(keys, ["root", "fire_menu", "ramp_menu", "eye_menu"]);
        assert_eq!(walk[2].keys, ["root", "fire_menu", "ramp_menu"]);
        assert_eq!(walk[2].depth, 2);
    }

    #[test]
    fn a_screen_with_two_parents_is_rejected() {
        let mut doc = example();
        let root = doc.screens.get_mut("root").unwrap();
        root.items.insert(
            0,
            Item {
                kind: ItemType::Submenu,
                render: "Ramping".into(),
                key: None,
                target: Some("ramp_menu".into()),
                hidden_when: None,
                disabled_when: None,
            },
        );
        let err = doc.check().unwrap_err().to_string();
        assert!(err.contains("ramp_menu"), "{err}");
    }

    #[test]
    fn a_setting_on_no_screen_is_rejected() {
        let mut doc = example();
        let fire = doc.screens.get_mut("fire_menu").unwrap();
        fire.items
            .retain(|i| i.key.as_deref() != Some("profile.fire_rate_cap"));
        let err = doc.check().unwrap_err().to_string();
        assert!(err.contains("profile.fire_rate_cap"), "{err}");
    }

    #[test]
    fn a_cycle_is_rejected() {
        let mut doc = example();
        let ramp = doc.screens.get_mut("ramp_menu").unwrap();
        ramp.items.insert(
            0,
            Item {
                kind: ItemType::Submenu,
                render: "Fire".into(),
                key: None,
                target: Some("fire_menu".into()),
                hidden_when: None,
                disabled_when: None,
            },
        );
        assert!(doc.check().is_err());
    }
}

//! The application document: the typed model of schema version 2, the rules that the schema
//! cannot state, and the resolution of an entry for each runtime.

use std::path::Path;

use anyhow::{bail, Context, Result};
use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::Value;

use crate::media::Media;
use crate::schema;
use crate::util::read_text;

/// The runtime key of the board in the manual. An app runtime cannot use it.
pub const RESERVED_RUNTIME: &str = "board";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub schema_version: String,
    pub app: App,
    #[serde(default)]
    pub groups: IndexMap<String, Group>,
    pub runtimes: IndexMap<String, Runtime>,
    #[serde(default)]
    pub screens: IndexMap<String, Screen>,
    #[serde(default)]
    pub roots: IndexMap<String, String>,
    pub settings: IndexMap<String, Entry>,
    #[serde(default)]
    pub topics: IndexMap<String, Entry>,
    #[serde(default)]
    pub features: IndexMap<String, Feature>,
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
pub struct Group {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Runtime {
    pub name: String,
    pub group: Option<String>,
    pub description: Option<String>,
    pub since: Option<String>,
    #[serde(default)]
    pub media: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Screen {
    pub title: String,
    pub description: Option<String>,
    pub runtimes: Vec<String>,
    pub items: Vec<Item>,
    #[serde(default)]
    pub media: Vec<String>,
    pub variants: Option<IndexMap<String, ScreenVariant>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenVariant {
    pub title: Option<String>,
    pub description: Option<String>,
    pub items: Option<Vec<Item>>,
    #[serde(default)]
    pub media: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemType {
    Setting,
    Screen,
    Action,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    #[serde(rename = "type")]
    pub kind: ItemType,
    pub runtimes: Option<Vec<String>>,
    pub key: Option<String>,
    #[serde(rename = "ref")]
    pub target: Option<String>,
    pub text: Option<String>,
    pub description: Option<String>,
    pub topic: Option<String>,
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
    pub app_only: bool,
    pub since: Option<String>,
    pub steps: Option<Vec<String>>,
    #[serde(default)]
    pub media: Vec<String>,
    #[serde(default)]
    pub related: Vec<String>,
    pub variants: Option<IndexMap<String, Variant>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Variant {
    pub absent: Option<bool>,
    pub title: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "where")]
    pub place: Option<String>,
    pub editable: Option<bool>,
    pub since: Option<String>,
    pub steps: Option<Vec<String>>,
    #[serde(default)]
    pub media: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Feature {
    pub title: String,
    pub description: Option<String>,
    pub support: IndexMap<String, Support>,
    #[serde(default)]
    pub related: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Yes,
    Limited,
    No,
}

impl Level {
    /// The word for the level in the matrix.
    pub fn word(self) -> &'static str {
        match self {
            Level::Yes => "Yes",
            Level::Limited => "Limited",
            Level::No => "No",
        }
    }

    /// The class of the cell in the matrix.
    pub fn class(self) -> &'static str {
        match self {
            Level::Yes => "yes",
            Level::Limited => "limited",
            Level::No => "no",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Support {
    Level(Level),
    Detail {
        level: Level,
        note: Option<String>,
        since: Option<String>,
    },
}

impl Support {
    pub fn level(&self) -> Level {
        match self {
            Support::Level(l) => *l,
            Support::Detail { level, .. } => *level,
        }
    }

    pub fn note(&self) -> Option<&str> {
        match self {
            Support::Level(_) => None,
            Support::Detail { note, .. } => note.as_deref(),
        }
    }

    pub fn since(&self) -> Option<&str> {
        match self {
            Support::Level(_) => None,
            Support::Detail { since, .. } => since.as_deref(),
        }
    }
}

/// The words of an entry for one runtime, after the variants are applied. Two runtimes
/// with equal resolved words share one block in the manual.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved<'a> {
    pub title: &'a str,
    pub description: &'a str,
    pub place: Option<&'a str>,
    pub editable: bool,
    pub since: Option<&'a str>,
    pub steps: Option<&'a [String]>,
    pub media: &'a [String],
}

/// One block of the manual: the resolved words, and the runtimes that share them, in the
/// order of the document.
pub struct Block<'a> {
    pub runtimes: Vec<&'a str>,
    pub words: Resolved<'a>,
}

/// The words of a screen for one runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenWords<'a> {
    pub title: &'a str,
    pub description: Option<&'a str>,
    pub items: &'a [Item],
    pub media: &'a [String],
}

/// One screen in the walk of a selector, with the keys from the root to it.
pub struct PlacedScreen<'a> {
    pub key: &'a str,
    pub screen: &'a Screen,
    pub keys: Vec<&'a str>,
    /// The item text that opens this screen from its parent.
    pub via: Option<&'a str>,
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

    /// The rules that JSON Schema cannot state. See docs/app-schema.md.
    pub fn check(&self) -> Result<()> {
        if self.runtimes.contains_key(RESERVED_RUNTIME) {
            bail!("the runtime key `{RESERVED_RUNTIME}` is reserved for the board");
        }
        for g in self.groups.keys() {
            if self.runtimes.contains_key(g) {
                bail!("`{g}` is a group key and a runtime key, and a selector must name one thing");
            }
            if !self
                .runtimes
                .values()
                .any(|r| r.group.as_deref() == Some(g))
            {
                bail!("the group `{g}` has no runtime");
            }
        }
        for (key, rt) in &self.runtimes {
            if let Some(g) = &rt.group {
                if !self.groups.contains_key(g) {
                    bail!("the runtime `{key}` names the group `{g}`, and there is no such group");
                }
            }
            self.media_exist(&rt.media, &format!("the runtime `{key}`"))?;
        }
        self.media_exist(&self.app.media, "the app")?;
        for (key, entry) in self.settings.iter().chain(self.topics.iter()) {
            self.media_exist(&entry.media, key)?;
            for (sel, v) in entry.variants.iter().flatten() {
                self.selector_exists(sel, key)?;
                self.media_exist(&v.media, key)?;
            }
        }
        for (key, screen) in &self.screens {
            for sel in &screen.runtimes {
                self.selector_exists(sel, &format!("the screen `{key}`"))?;
            }
            self.media_exist(&screen.media, &format!("the screen `{key}`"))?;
            for (sel, v) in screen.variants.iter().flatten() {
                self.selector_exists(sel, &format!("the screen `{key}`"))?;
                self.media_exist(&v.media, &format!("the screen `{key}`"))?;
                if let Some(items) = &v.items {
                    self.items_ok(items, key)?;
                }
            }
            self.items_ok(&screen.items, key)?;
        }
        for (sel, root) in &self.roots {
            self.selector_exists(sel, "`roots`")?;
            let screen = match self.screens.get(root) {
                Some(s) => s,
                None => bail!(
                    "`roots` names the screen `{root}` for `{sel}`, and there is no such screen"
                ),
            };
            if !self.screen_covers(screen, sel) {
                bail!("the root screen `{root}` does not exist on `{sel}`");
            }
            let placed = self.walk(sel)?;
            for (key, s) in &self.screens {
                if self.screen_covers(s, sel) && !placed.iter().any(|p| p.key == key) {
                    bail!("the screen `{key}` exists on `{sel}`, and no path from the root `{root}` reaches it");
                }
            }
        }
        for (key, f) in &self.features {
            for sel in f.support.keys() {
                self.selector_exists(sel, &format!("the feature `{key}`"))?;
            }
            for rt in self.runtimes.keys() {
                if self.support_of(f, rt).is_none() {
                    bail!("the feature `{key}` states no support for the runtime `{rt}`");
                }
            }
        }
        Ok(())
    }

    /// Each setting of the app must be a setting of the firmware, or be marked app_only.
    pub fn check_against(&self, firmware_has_setting: impl Fn(&str) -> bool) -> Result<()> {
        for (key, e) in &self.settings {
            if !e.app_only && !firmware_has_setting(key) {
                bail!("the app document describes the setting `{key}`, and the firmware document has no such setting; mark it app_only if the app alone has it");
            }
            if e.app_only && firmware_has_setting(key) {
                bail!("the app document marks the setting `{key}` app_only, and the firmware document has it");
            }
        }
        Ok(())
    }

    fn items_ok(&self, items: &[Item], screen: &str) -> Result<()> {
        for it in items {
            for sel in it.runtimes.iter().flatten() {
                self.selector_exists(sel, &format!("an item of the screen `{screen}`"))?;
            }
            match it.kind {
                ItemType::Setting => {
                    let k = it.key.as_deref().unwrap_or_default();
                    if !self.settings.contains_key(k) {
                        bail!("the screen `{screen}` names the setting `{k}`, and there is no such setting");
                    }
                }
                ItemType::Screen => {
                    let t = it.target.as_deref().unwrap_or_default();
                    if !self.screens.contains_key(t) {
                        bail!("the screen `{screen}` opens `{t}`, and there is no such screen");
                    }
                }
                ItemType::Action => {
                    if let Some(t) = &it.topic {
                        if !self.topics.contains_key(t) {
                            bail!("the screen `{screen}` names the topic `{t}`, and there is no such topic");
                        }
                    }
                }
                ItemType::Info => {}
            }
        }
        Ok(())
    }

    fn selector_exists(&self, sel: &str, owner: &str) -> Result<()> {
        if self.runtimes.contains_key(sel) || self.groups.contains_key(sel) {
            return Ok(());
        }
        bail!("{owner} names `{sel}`, and there is no such runtime or group")
    }

    fn media_exist(&self, keys: &[String], owner: &str) -> Result<()> {
        for k in keys {
            if !self.media.contains_key(k) {
                bail!("`{owner}` shows the media `{k}`, and there is no such media item");
            }
        }
        Ok(())
    }

    /// The group of a runtime.
    pub fn group_of(&self, runtime: &str) -> Option<&str> {
        self.runtimes.get(runtime)?.group.as_deref()
    }

    /// True when a selector covers a runtime: the selector is the runtime, or its group.
    pub fn selects(&self, selector: &str, runtime: &str) -> bool {
        selector == runtime || self.group_of(runtime) == Some(selector)
    }

    /// True when one of the selectors covers the runtime.
    pub fn any_selects(&self, selectors: &[String], runtime: &str) -> bool {
        selectors.iter().any(|s| self.selects(s, runtime))
    }

    /// The runtimes that a list of selectors covers, in document order.
    pub fn runtimes_of(&self, selectors: &[String]) -> Vec<&str> {
        self.runtimes
            .keys()
            .filter(|rt| self.any_selects(selectors, rt))
            .map(String::as_str)
            .collect()
    }

    /// True when a screen exists on a selector: on the runtime, or on one runtime of the
    /// group.
    fn screen_covers(&self, screen: &Screen, selector: &str) -> bool {
        self.runtimes_of(&screen.runtimes)
            .iter()
            .any(|rt| self.selects(selector, rt))
    }

    /// The entry of a shared key, as a setting or as a topic.
    pub fn entry(&self, key: &str) -> Option<&Entry> {
        self.settings.get(key).or_else(|| self.topics.get(key))
    }

    /// The words of an entry for one runtime, or None when the entry is absent there. A
    /// runtime variant wins over a group variant, and a group variant wins over the top.
    pub fn resolve<'a>(&'a self, entry: &'a Entry, runtime: &str) -> Option<Resolved<'a>> {
        let variants = entry.variants.as_ref();
        let group_v = self
            .group_of(runtime)
            .and_then(|g| variants.and_then(|m| m.get(g)));
        let rt_v = variants.and_then(|m| m.get(runtime));
        let absent = match (rt_v, group_v) {
            (Some(v), _) => v.absent.unwrap_or(false),
            (None, Some(v)) => v.absent.unwrap_or(false),
            (None, None) => false,
        };
        if absent {
            return None;
        }
        let mut r = Resolved {
            title: &entry.title,
            description: &entry.description,
            place: entry.place.as_deref(),
            editable: entry.editable.unwrap_or(true),
            since: entry.since.as_deref(),
            steps: entry.steps.as_deref(),
            media: &entry.media,
        };
        for v in [group_v, rt_v].into_iter().flatten() {
            if let Some(t) = &v.title {
                r.title = t;
            }
            if let Some(d) = &v.description {
                r.description = d;
            }
            if let Some(p) = &v.place {
                r.place = Some(p);
            }
            if let Some(e) = v.editable {
                r.editable = e;
            }
            if let Some(s) = &v.since {
                r.since = Some(s);
            }
            if let Some(s) = &v.steps {
                r.steps = Some(s);
            }
            if !v.media.is_empty() {
                r.media = &v.media;
            }
        }
        Some(r)
    }

    /// The blocks of an entry: the runtimes with equal words share one block, in the order
    /// of the first runtime of each block.
    pub fn blocks<'a>(&'a self, entry: &'a Entry) -> Vec<Block<'a>> {
        let mut out: Vec<Block<'a>> = Vec::new();
        for rt in self.runtimes.keys() {
            let words = match self.resolve(entry, rt) {
                Some(w) => w,
                None => continue,
            };
            match out.iter_mut().find(|b| b.words == words) {
                Some(b) => b.runtimes.push(rt),
                None => out.push(Block {
                    runtimes: vec![rt],
                    words,
                }),
            }
        }
        out
    }

    /// The words of a screen for one runtime.
    pub fn screen_words<'a>(&'a self, screen: &'a Screen, runtime: &str) -> ScreenWords<'a> {
        let mut w = ScreenWords {
            title: &screen.title,
            description: screen.description.as_deref(),
            items: &screen.items,
            media: &screen.media,
        };
        let group_v = self
            .group_of(runtime)
            .and_then(|g| screen.variants.as_ref()?.get(g));
        let rt_v = screen.variants.as_ref().and_then(|m| m.get(runtime));
        for v in [group_v, rt_v].into_iter().flatten() {
            if let Some(t) = &v.title {
                w.title = t;
            }
            if let Some(d) = &v.description {
                w.description = Some(d);
            }
            if let Some(i) = &v.items {
                w.items = i;
            }
            if !v.media.is_empty() {
                w.media = &v.media;
            }
        }
        w
    }

    /// The blocks of a screen: the runtimes that have it, with equal words merged.
    pub fn screen_blocks<'a>(&'a self, screen: &'a Screen) -> Vec<(Vec<&'a str>, ScreenWords<'a>)> {
        let mut out: Vec<(Vec<&'a str>, ScreenWords<'a>)> = Vec::new();
        for rt in self.runtimes_of(&screen.runtimes) {
            let w = self.screen_words(screen, rt);
            match out.iter_mut().find(|(_, x)| *x == w) {
                Some((rts, _)) => rts.push(rt),
                None => out.push((vec![rt], w)),
            }
        }
        out
    }

    /// The support of a feature on a runtime.
    pub fn support_of<'a>(&self, f: &'a Feature, runtime: &str) -> Option<&'a Support> {
        if let Some(s) = f.support.get(runtime) {
            return Some(s);
        }
        let g = self.group_of(runtime)?;
        f.support.get(g)
    }

    /// The screens of a selector, in the order of a depth-first walk from its root. The
    /// items of a screen that open other screens give the order of the children.
    pub fn walk(&self, selector: &str) -> Result<Vec<PlacedScreen<'_>>> {
        let root = match self.roots.get(selector) {
            Some(r) => r.as_str(),
            None => bail!("`roots` names no screen for `{selector}`"),
        };
        let mut out = Vec::new();
        let mut keys = Vec::new();
        self.walk_into(root, selector, None, &mut keys, &mut out)?;
        Ok(out)
    }

    fn walk_into<'a>(
        &'a self,
        key: &'a str,
        selector: &str,
        via: Option<&'a str>,
        keys: &mut Vec<&'a str>,
        out: &mut Vec<PlacedScreen<'a>>,
    ) -> Result<()> {
        let screen = match self.screens.get(key) {
            Some(s) => s,
            None => bail!("an item opens the screen `{key}`, and there is no such screen"),
        };
        if out.iter().any(|p| p.key == key) {
            // A screen that two items open is placed once, where the walk met it first.
            return Ok(());
        }
        keys.push(key);
        out.push(PlacedScreen {
            key,
            screen,
            keys: keys.clone(),
            via,
        });
        for (t, via) in self.children_of(screen, selector) {
            self.walk_into(t, selector, Some(via), keys, out)?;
        }
        keys.pop();
        Ok(())
    }

    /// The runtimes of a screen that a selector covers, in document order.
    pub fn covered_runtimes(&self, screen: &Screen, selector: &str) -> Vec<&str> {
        self.runtimes_of(&screen.runtimes)
            .into_iter()
            .filter(|rt| self.selects(selector, rt))
            .collect()
    }

    /// The screens that the items of a screen open on a selector, with the text of the
    /// control, once each, in the order the items of each covered runtime give them. A
    /// screen that only one runtime's variant items open is a child too.
    pub fn children_of<'a>(
        &'a self,
        screen: &'a Screen,
        selector: &str,
    ) -> Vec<(&'a str, &'a str)> {
        let mut out: Vec<(&str, &str)> = Vec::new();
        for rt in self.covered_runtimes(screen, selector) {
            for it in self.screen_words(screen, rt).items {
                if it.kind != ItemType::Screen {
                    continue;
                }
                let t = match it.target.as_deref() {
                    Some(t) => t,
                    None => continue,
                };
                let child = match self.screens.get(t) {
                    Some(c) => c,
                    None => continue,
                };
                if self.screen_covers(child, selector) && !out.iter().any(|(k, _)| *k == t) {
                    out.push((t, it.text.as_deref().unwrap_or("")));
                }
            }
        }
        out
    }

    /// The name of a selector: the name of the group, or of the runtime.
    pub fn selector_name(&self, selector: &str) -> String {
        if let Some(g) = self.groups.get(selector) {
            return g.name.clone();
        }
        self.runtimes
            .get(selector)
            .map_or_else(|| selector.to_string(), |r| r.name.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc() -> Document {
        serde_json::from_value(serde_json::json!({
            "schema_version": "2.0.0",
            "app": { "name": "App", "version": "1.0.0" },
            "groups": { "shared": { "name": "Shared" }, "watch": { "name": "Watch" } },
            "runtimes": {
                "android": { "name": "Android", "group": "shared" },
                "ios": { "name": "iOS", "group": "shared" },
                "wear": { "name": "Wear OS", "group": "watch" },
                "zepp": { "name": "Zepp OS", "group": "watch" }
            },
            "roots": { "shared": "home", "watch": "list" },
            "screens": {
                "home": { "title": "Home", "runtimes": ["shared"], "items": [
                    { "type": "screen", "ref": "edit", "text": "Edit" },
                    { "type": "setting", "key": "a.b" }
                ] },
                "edit": { "title": "Edit", "runtimes": ["shared"], "items": [] },
                "list": { "title": "List", "runtimes": ["watch"], "items": [
                    { "type": "setting", "key": "a.b", "runtimes": ["wear"] }
                ] }
            },
            "settings": {
                "a.b": { "title": "B", "description": "All.", "where": "X > Y" },
                "a.c": { "title": "C", "description": "Some.", "variants": {
                    "watch": { "absent": true },
                    "wear": { "description": "Wear has it." }
                } }
            },
            "features": {
                "feature.x": { "title": "X", "support": { "shared": "yes", "watch": { "level": "limited", "note": "Part." }, "zepp": "no" } }
            }
        }))
        .unwrap()
    }

    #[test]
    fn the_document_passes_the_rules() {
        doc().check().unwrap();
    }

    #[test]
    fn equal_words_share_one_block() {
        let d = doc();
        let b = d.blocks(d.entry("a.b").unwrap());
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].runtimes, ["android", "ios", "wear", "zepp"]);
    }

    #[test]
    fn a_runtime_variant_overrides_a_group_absence() {
        let d = doc();
        let b = d.blocks(d.entry("a.c").unwrap());
        assert_eq!(b.len(), 2);
        assert_eq!(b[0].runtimes, ["android", "ios"]);
        assert_eq!(b[1].runtimes, ["wear"]);
        assert_eq!(b[1].words.description, "Wear has it.");
        assert!(d.resolve(d.entry("a.c").unwrap(), "zepp").is_none());
    }

    #[test]
    fn support_resolves_runtime_over_group() {
        let d = doc();
        let f = &d.features["feature.x"];
        assert_eq!(d.support_of(f, "android").unwrap().level(), Level::Yes);
        assert_eq!(d.support_of(f, "wear").unwrap().level(), Level::Limited);
        assert_eq!(d.support_of(f, "zepp").unwrap().level(), Level::No);
    }

    #[test]
    fn the_walk_follows_the_screen_items() {
        let d = doc();
        let w = d.walk("shared").unwrap();
        let keys: Vec<&str> = w.iter().map(|p| p.key).collect();
        assert_eq!(keys, ["home", "edit"]);
        assert_eq!(w[1].via, Some("Edit"));
        assert_eq!(w[1].keys, ["home", "edit"]);
        let w = d.walk("watch").unwrap();
        assert_eq!(w.len(), 1);
    }

    #[test]
    fn a_screen_that_only_a_variant_opens_is_reached() {
        let mut d = doc();
        d.screens.insert(
            "x".into(),
            Screen {
                title: "X".into(),
                description: None,
                runtimes: vec!["zepp".into()],
                items: vec![],
                media: vec![],
                variants: None,
            },
        );
        let list = d.screens.get_mut("list").unwrap();
        let mut variants = IndexMap::new();
        variants.insert(
            "zepp".to_string(),
            ScreenVariant {
                title: None,
                description: None,
                items: Some(vec![Item {
                    kind: ItemType::Screen,
                    runtimes: None,
                    key: None,
                    target: Some("x".into()),
                    text: Some("Open X".into()),
                    description: None,
                    topic: None,
                }]),
                media: vec![],
            },
        );
        list.variants = Some(variants);
        d.check().unwrap();
        let w = d.walk("watch").unwrap();
        assert_eq!(w.iter().map(|p| p.key).collect::<Vec<_>>(), ["list", "x"]);
        assert_eq!(w[1].via, Some("Open X"));
    }

    #[test]
    fn a_screen_that_two_items_open_is_placed_once_under_the_first() {
        let mut d = doc();
        let open_edit = Item {
            kind: ItemType::Screen,
            runtimes: None,
            key: None,
            target: Some("edit".into()),
            text: Some("Edit again".into()),
            description: None,
            topic: None,
        };
        d.screens.insert(
            "other".into(),
            Screen {
                title: "Other".into(),
                description: None,
                runtimes: vec!["shared".into()],
                items: vec![open_edit],
                media: vec![],
                variants: None,
            },
        );
        d.screens.get_mut("home").unwrap().items.push(Item {
            kind: ItemType::Screen,
            runtimes: None,
            key: None,
            target: Some("other".into()),
            text: Some("Other".into()),
            description: None,
            topic: None,
        });
        d.check().unwrap();
        let w = d.walk("shared").unwrap();
        assert_eq!(
            w.iter().map(|p| p.key).collect::<Vec<_>>(),
            ["home", "edit", "other"]
        );
        assert_eq!(w[1].keys, ["home", "edit"]);
    }

    #[test]
    fn a_group_key_cannot_be_a_runtime_key() {
        let mut d = doc();
        d.groups.insert(
            "wear".into(),
            Group {
                name: "Wear group".into(),
                description: None,
            },
        );
        d.runtimes.get_mut("zepp").unwrap().group = Some("wear".into());
        let err = d.check().unwrap_err().to_string();
        assert!(err.contains("group key and a runtime key"), "{err}");
    }

    #[test]
    fn a_group_with_no_runtime_is_rejected() {
        let mut d = doc();
        d.groups.insert(
            "empty".into(),
            Group {
                name: "Empty".into(),
                description: None,
            },
        );
        let err = d.check().unwrap_err().to_string();
        assert!(err.contains("has no runtime"), "{err}");
    }

    #[test]
    fn a_typo_in_a_support_object_is_rejected() {
        let v = serde_json::json!({ "level": "yes", "notes": "x" });
        assert!(serde_json::from_value::<Support>(v).is_err());
    }

    #[test]
    fn an_unreachable_screen_is_rejected() {
        let mut d = doc();
        d.screens.insert(
            "lost".into(),
            Screen {
                title: "Lost".into(),
                description: None,
                runtimes: vec!["shared".into()],
                items: vec![],
                media: vec![],
                variants: None,
            },
        );
        let err = d.check().unwrap_err().to_string();
        assert!(err.contains("lost"), "{err}");
    }

    #[test]
    fn a_feature_must_cover_every_runtime() {
        let mut d = doc();
        d.features
            .get_mut("feature.x")
            .unwrap()
            .support
            .shift_remove("watch");
        let err = d.check().unwrap_err().to_string();
        assert!(err.contains("wear"), "{err}");
    }

    #[test]
    fn app_only_settings_pair_with_the_firmware() {
        let d = doc();
        assert!(d.check_against(|k| k == "a.b" || k == "a.c").is_ok());
        let err = d.check_against(|k| k == "a.b").unwrap_err().to_string();
        assert!(err.contains("a.c"), "{err}");
    }

    #[test]
    fn the_board_key_is_reserved() {
        let mut d = doc();
        d.runtimes.insert(
            "board".into(),
            Runtime {
                name: "Board".into(),
                group: None,
                description: None,
                since: None,
                media: vec![],
            },
        );
        assert!(d.check().unwrap_err().to_string().contains("reserved"));
    }
}

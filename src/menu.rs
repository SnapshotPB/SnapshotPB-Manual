//! The on-device menu: the screens and the rows of `menu_tree.yaml`, and the words of
//! `menu_lang.yaml`.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde_yaml::{Mapping, Value};

use crate::util::read_text;

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Enum,
    Int,
    Toggle,
    Submenu,
    Back,
}

#[derive(Debug, Clone)]
pub struct Option_ {
    pub value: String,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct Item {
    pub kind: Kind,
    pub render: String,
    pub bind: Option<String>,
    pub options: Vec<Option_>,
    pub reference: Option<String>,
    pub min: Option<i64>,
    pub max: Option<i64>,
    pub step: Option<i64>,
    pub unit: Option<String>,
    pub decimals: u32,
    pub zero_text: Option<String>,
    pub disabled_when: Option<String>,
    pub hidden_when: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Screen {
    pub key: String,
    pub title: String,
    /// The titles from the root screen to this one.
    pub path: Vec<String>,
    pub items: Vec<Item>,
}

pub struct MenuModel {
    /// The screens in the order of a walk from the root.
    pub screens: Vec<Screen>,
}

impl MenuModel {
    pub fn load(path: &Path) -> Result<MenuModel> {
        let text = read_text(path)?;
        let doc: Value =
            serde_yaml::from_str(&text).with_context(|| format!("YAML of {}", path.display()))?;
        let map = match doc.as_mapping() {
            Some(m) => m,
            None => bail!("{} is not a mapping of screens", path.display()),
        };
        let mut raw: BTreeMap<String, (String, Vec<Item>)> = BTreeMap::new();
        for (k, v) in map {
            let key = str_of(k).unwrap_or_default();
            let screen = v
                .as_mapping()
                .with_context(|| format!("screen `{key}` is not a mapping"))?;
            let title = get_str(screen, "title").unwrap_or_else(|| key.clone());
            let mut items = Vec::new();
            for it in get(screen, "items")
                .and_then(Value::as_sequence)
                .into_iter()
                .flatten()
            {
                let m = it
                    .as_mapping()
                    .with_context(|| format!("an item of `{key}` is not a mapping"))?;
                items.push(parse_item(m).with_context(|| format!("screen `{key}`"))?);
            }
            raw.insert(key, (title, items));
        }
        if !raw.contains_key("root") {
            bail!("{} has no `root` screen", path.display());
        }
        let mut screens = Vec::new();
        let mut seen = Vec::new();
        walk("root", &mut Vec::new(), &raw, &mut screens, &mut seen)?;
        Ok(MenuModel { screens })
    }

    pub fn screen(&self, key: &str) -> Option<&Screen> {
        self.screens.iter().find(|s| s.key == key)
    }
}

fn walk(
    key: &str,
    path: &mut Vec<String>,
    raw: &BTreeMap<String, (String, Vec<Item>)>,
    out: &mut Vec<Screen>,
    seen: &mut Vec<String>,
) -> Result<()> {
    let (title, items) = match raw.get(key) {
        Some(s) => s,
        None => bail!("a submenu row refers to `{key}`, and there is no such screen"),
    };
    if seen.iter().any(|s| s == key) {
        bail!("the screen `{key}` is its own ancestor");
    }
    seen.push(key.to_string());
    path.push(title.clone());
    out.push(Screen {
        key: key.to_string(),
        title: title.clone(),
        path: path.clone(),
        items: items.clone(),
    });
    for it in items {
        if it.kind == Kind::Submenu {
            if let Some(r) = &it.reference {
                walk(r, path, raw, out, seen)?;
            }
        }
    }
    path.pop();
    seen.pop();
    Ok(())
}

fn get<'a>(m: &'a Mapping, k: &str) -> Option<&'a Value> {
    m.get(Value::String(k.to_string()))
}

fn get_str(m: &Mapping, k: &str) -> Option<String> {
    get(m, k).and_then(str_of)
}

fn get_i64(m: &Mapping, k: &str) -> Option<i64> {
    get(m, k).and_then(Value::as_i64)
}

/// A scalar as text: `9V`, `OFF`, `1` and `2S_LIPO` all read as the text that they show.
fn str_of(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn parse_item(m: &Mapping) -> Result<Item> {
    let kind = match get_str(m, "type").as_deref() {
        Some("enum") => Kind::Enum,
        Some("int") => Kind::Int,
        Some("toggle") => Kind::Toggle,
        Some("submenu") => Kind::Submenu,
        Some("back") => Kind::Back,
        other => bail!("unknown item type {other:?}"),
    };
    let render = get_str(m, "render")
        .or_else(|| get_str(m, "label"))
        .unwrap_or_default();
    let mut options = Vec::new();
    for o in get(m, "options")
        .and_then(Value::as_sequence)
        .into_iter()
        .flatten()
    {
        match o {
            Value::Mapping(om) => {
                let value = get_str(om, "value").unwrap_or_default();
                let text = get_str(om, "text").unwrap_or_else(|| value.clone());
                options.push(Option_ { value, text });
            }
            other => {
                let value = str_of(other).unwrap_or_default();
                options.push(Option_ {
                    text: value.clone(),
                    value,
                });
            }
        }
    }
    Ok(Item {
        kind,
        render,
        bind: get_str(m, "bind"),
        options,
        reference: get_str(m, "ref"),
        min: get_i64(m, "min"),
        max: get_i64(m, "max"),
        step: get_i64(m, "step"),
        unit: get_str(m, "unit"),
        decimals: get_i64(m, "decimals").unwrap_or(0) as u32,
        zero_text: get_str(m, "zero_text"),
        disabled_when: get_str(m, "disabled_when"),
        hidden_when: get_str(m, "hidden_when"),
    })
}

/// The words of the panel in each language.
pub struct Lang {
    /// `(code, english -> translated)` in file order. English is not a block.
    pub languages: Vec<(String, BTreeMap<String, String>)>,
    /// The strings that the C code draws, by their key.
    pub named: Vec<(String, String)>,
}

impl Lang {
    pub fn load(path: &Path) -> Result<Lang> {
        let text = read_text(path)?;
        let doc: Value =
            serde_yaml::from_str(&text).with_context(|| format!("YAML of {}", path.display()))?;
        let map = match doc.as_mapping() {
            Some(m) => m,
            None => bail!("{} is not a mapping", path.display()),
        };
        let mut languages = Vec::new();
        for (k, v) in get(map, "languages")
            .and_then(Value::as_mapping)
            .into_iter()
            .flatten()
        {
            let code = str_of(k).unwrap_or_default();
            let mut words = BTreeMap::new();
            for (ek, tv) in v.as_mapping().into_iter().flatten() {
                if let (Some(e), Some(t)) = (str_of(ek), str_of(tv)) {
                    words.insert(e, t);
                }
            }
            languages.push((code, words));
        }
        let mut named = Vec::new();
        for (k, v) in get(map, "named")
            .and_then(Value::as_mapping)
            .into_iter()
            .flatten()
        {
            if let (Some(key), Some(t)) = (str_of(k), str_of(v)) {
                named.push((key, t));
            }
        }
        if languages.is_empty() {
            bail!("{} has no `languages` block", path.display());
        }
        Ok(Lang { languages, named })
    }

    /// The word of a language for an English string. A string with no letter is the same in
    /// every language.
    pub fn word(&self, code: &str, english: &str) -> String {
        if !english.chars().any(char::is_alphabetic) {
            return english.to_string();
        }
        self.languages
            .iter()
            .find(|(c, _)| c == code)
            .and_then(|(_, w)| w.get(english).cloned())
            .unwrap_or_else(|| format!("({english})"))
    }
}

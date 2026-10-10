//! The blocks that the generator writes from the documents: the settings reference with its
//! tabs, the menu tree, the controls, the interactions, the topics, the board figures and
//! the provenance.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::LazyLock;

use crate::app;
use crate::board::BoardInfo;
use crate::firmware::{self, ItemType, Placed, Scope, SettingType};
use crate::manifest::Loaded;
use crate::media;
use crate::tree;
use crate::util::{esc, slug};
use anyhow::{bail, Context, Result};
use regex::Regex;

/// The key and the tab name of the board itself, which is the first tab of every group.
pub const BOARD_TARGET: &str = app::RESERVED_TARGET;

pub struct Ctx<'a> {
    pub loaded: &'a Loaded,
    pub fw: &'a firmware::Document,
    pub fw_dir: &'a Path,
    pub app: Option<&'a app::Document>,
    pub app_dir: &'a Path,
    pub board: &'a BoardInfo,
    pub placed: Vec<Placed<'a>>,
    pub values: BTreeMap<String, String>,
    /// `(what, where, version)` rows.
    pub provenance: Vec<(String, String, String)>,
}

/// The id of the section of a setting.
pub fn setting_id(key: &str) -> String {
    format!("setting-{}", slug(key))
}

/// The id of the section of a topic.
pub fn topic_id(key: &str) -> String {
    format!("topic-{}", slug(key.strip_prefix("topic.").unwrap_or(key)))
}

impl<'a> Ctx<'a> {
    /// The tab targets of the page: the board, then each target of the app document.
    pub fn targets(&self) -> Vec<(String, String)> {
        let mut out = vec![(BOARD_TARGET.to_string(), "Board".to_string())];
        if let Some(app) = self.app {
            for (k, t) in &app.targets {
                out.push((k.clone(), t.name.clone()));
            }
        }
        out
    }

    /// Runs one `{{name args}}` directive.
    pub fn directive(&self, name: &str, args: &str) -> Result<String> {
        match name {
            "menu" => self.menu_reference(),
            "menu-map" => Ok(self.menu_map()),
            "controls" => self.controls(),
            "interactions" => Ok(self.interactions()),
            "home" => self.home(),
            "topic" => self.topic(args.trim()),
            "topics" => self.topics(),
            "targets" => Ok(self.app_targets()),
            "media" => self.media_item(args.trim()),
            "board" => Ok(self.board_table()),
            "provenance" => Ok(self.provenance_table()),
            "image" => self.image(args),
            other => bail!("unknown directive `{{{{{other}}}}}`"),
        }
    }

    pub fn value(&self, key: &str) -> Result<String> {
        if let Some(v) = self.values.get(key) {
            return Ok(v.clone());
        }
        if let Some(rest) = key.strip_prefix("control.") {
            if let Some((ck, field)) = rest.split_once('.') {
                let c = match self.fw.controls.get(ck) {
                    Some(c) => c,
                    None => bail!("{{{{value {key}}}}}: no control `{ck}`"),
                };
                return match field {
                    "name" => Ok(c.name.clone()),
                    "hold_ms" => c
                        .events
                        .values()
                        .find_map(|e| e.hold_ms)
                        .map(|n| n.to_string())
                        .with_context(|| {
                            format!("{{{{value {key}}}}}: the control has no long press")
                        }),
                    other => bail!("{{{{value {key}}}}}: a control has no `{other}`"),
                };
            }
        }
        bail!("{{{{value {key}}}}}: unknown value")
    }

    // ----- the settings reference -----

    fn menu_reference(&self) -> Result<String> {
        let mut out = String::new();
        for p in &self.placed {
            out.push_str(&self.screen_section(p)?);
        }
        Ok(out)
    }

    fn screen_section(&self, p: &Placed) -> Result<String> {
        let mut out = String::new();
        out.push_str(&format!(
            "<h3 id=\"{}\">{}</h3>\n",
            tree::screen_id(p.key),
            esc(&p.screen.title)
        ));
        out.push_str(&format!(
            "<p class=\"crumbs\">{}</p>\n",
            self.crumbs(&p.keys, None)
        ));
        if let Some(d) = &p.screen.description {
            out.push_str(&paras(d));
        }
        out.push_str(&self.fw_media(&p.screen.media)?);
        out.push_str("<table class=\"rows\"><thead><tr><th>Row</th><th>What it is</th></tr></thead><tbody>\n");
        for item in &p.screen.items {
            let what = match item.kind {
                ItemType::Setting => {
                    let key = item.key.as_deref().unwrap_or_default();
                    let s = &self.fw.settings[key];
                    format!(
                        "<a href=\"#{}\">{}</a>: {}",
                        setting_id(key),
                        esc(&s.title),
                        kind_words(s.kind)
                    )
                }
                ItemType::Submenu => {
                    let t = item.target.as_deref().unwrap_or_default();
                    let title = self.fw.screens.get(t).map_or("", |s| s.title.as_str());
                    format!(
                        "Opens the <a href=\"#{}\">{}</a> screen",
                        tree::screen_id(t),
                        esc(title)
                    )
                }
                ItemType::Back => {
                    if p.depth == 0 {
                        "Returns to the home screen".to_string()
                    } else {
                        let parent = p.keys[p.keys.len() - 2];
                        format!(
                            "Returns to the <a href=\"#{}\">{}</a> screen",
                            tree::screen_id(parent),
                            esc(self
                                .fw
                                .screens
                                .get(parent)
                                .map_or(parent, |s| s.title.as_str()))
                        )
                    }
                }
            };
            let mut when = String::new();
            if let Some(pr) = &item.hidden_when {
                when.push_str(&format!(
                    " <span class=\"when\">Hidden while {}.</span>",
                    esc(&self.predicate_words(pr))
                ));
            }
            if let Some(pr) = &item.disabled_when {
                when.push_str(&format!(
                    " <span class=\"when\">Does not apply while {}.</span>",
                    esc(&self.predicate_words(pr))
                ));
            }
            out.push_str(&format!(
                "<tr><td><b>{}</b></td><td>{what}{when}</td></tr>\n",
                esc(&item.render)
            ));
        }
        out.push_str("</tbody></table>\n");
        for item in &p.screen.items {
            if item.kind == ItemType::Setting {
                let key = item.key.as_deref().unwrap_or_default();
                out.push_str(
                    &self
                        .setting_section(key, item, p)
                        .with_context(|| format!("the setting `{key}`"))?,
                );
            }
        }
        Ok(out)
    }

    /// The breadcrumb line: the home screen, each screen on the path by key, and the row.
    fn crumbs(&self, keys: &[&str], last: Option<&str>) -> String {
        let mut parts: Vec<String> = Vec::new();
        parts.push(format!("<span>{}</span>", esc(&self.fw.home.title)));
        for k in keys {
            let title = self.fw.screens.get(*k).map_or(*k, |s| s.title.as_str());
            parts.push(format!(
                "<a href=\"#{}\">{}</a>",
                tree::screen_id(k),
                esc(title)
            ));
        }
        if let Some(l) = last {
            parts.push(format!("<b>{}</b>", esc(l)));
        }
        parts.join(" &rsaquo; ")
    }

    fn setting_section(&self, key: &str, item: &firmware::Item, p: &Placed) -> Result<String> {
        let s = &self.fw.settings[key];
        let id = setting_id(key);
        let mut out = String::new();
        out.push_str(&format!("<section class=\"setting\" id=\"{id}\">\n"));
        out.push_str(&format!(
            "<h4>{} <code class=\"key\">{}</code></h4>\n",
            esc(&s.title),
            esc(key)
        ));
        out.push_str(&format!(
            "<p class=\"crumbs\">{}</p>\n",
            self.crumbs(&p.keys, Some(&item.render))
        ));
        out.push_str("<dl class=\"facts\">\n");
        out.push_str(&format!(
            "<dt>Row</dt><dd><b>{}</b> <span class=\"kind\">{}</span></dd>\n",
            esc(&item.render),
            kind_words(s.kind)
        ));
        out.push_str(&format!("<dt>Values</dt><dd>{}</dd>\n", self.values_of(s)?));
        let default = self.default_of(s)?;
        if !default.is_empty() {
            out.push_str(&format!("<dt>Default</dt><dd>{default}</dd>\n"));
        }
        out.push_str(&format!(
            "<dt>Scope</dt><dd>{}</dd>\n",
            scope_words(s.scope)
        ));
        let mut when = Vec::new();
        if let Some(pr) = &item.hidden_when {
            when.push(format!(
                "The row is hidden while {}.",
                esc(&self.predicate_words(pr))
            ));
        }
        if let Some(pr) = &item.disabled_when {
            when.push(format!(
                "The row does not apply while {}.",
                esc(&self.predicate_words(pr))
            ));
        }
        if !when.is_empty() {
            out.push_str(&format!("<dt>When</dt><dd>{}</dd>\n", when.join(" ")));
        }
        out.push_str("</dl>\n");

        // The board panel: the words of the firmware.
        let mut board = paras(&s.description);
        if s.kind == SettingType::Enum {
            board.push_str("<table class=\"options\"><thead><tr><th>Option</th><th>What it does</th></tr></thead><tbody>\n");
            for o in &s.options {
                board.push_str(&format!(
                    "<tr><td><b>{}</b></td><td>{}</td></tr>\n",
                    esc(&o.text),
                    paras(&o.description)
                ));
            }
            board.push_str("</tbody></table>\n");
        }
        board.push_str(&self.fw_media(&s.media)?);
        board.push_str(&self.related(&s.related));
        out.push_str(&self.tabs(&id, Some(board), key)?);
        out.push_str("</section>\n");
        Ok(out)
    }

    /// A tab group: the board panel first, then one panel for each app target that has the
    /// key. With no app entry, the board panel stands alone.
    fn tabs(&self, id: &str, board_html: Option<String>, key: &str) -> Result<String> {
        let mut panels: Vec<(&str, String, String)> = Vec::new();
        if let Some(b) = board_html {
            panels.push((BOARD_TARGET, "Board".to_string(), b));
        }
        if let Some(app) = self.app {
            if let Some(entry) = app.entry(key) {
                for (tk, target, r) in app.resolve(entry) {
                    let mut html = String::new();
                    html.push_str(&format!("<p class=\"app-title\"><b>{}</b>", esc(r.title)));
                    if let Some(place) = r.place {
                        html.push_str(&format!(" <span class=\"where\">{}</span>", esc(place)));
                    }
                    html.push_str("</p>\n");
                    if !r.editable {
                        html.push_str("<p class=\"when\">The app shows the value and does not change it.</p>\n");
                    }
                    html.push_str(&paras(r.description));
                    for mk in r.media {
                        let m = &app.media[mk];
                        html.push_str(&m.html(self.loaded, self.app_dir)?);
                    }
                    panels.push((tk, target.name.clone(), html));
                }
            }
        }
        if panels.is_empty() {
            bail!("no document describes `{key}`");
        }
        if panels.len() == 1 {
            return Ok(format!("<div class=\"panel-only\">{}</div>\n", panels[0].2));
        }
        let mut out = format!("<div class=\"tabs\" data-tabs=\"{id}\">\n");
        for (i, (tk, name, _)) in panels.iter().enumerate() {
            out.push_str(&format!(
                "<input type=\"radio\" name=\"tabs-{id}\" id=\"tab-{id}-{tk}\" class=\"tab-input t-{tk}\"{}><label for=\"tab-{id}-{tk}\">{}</label>\n",
                if i == 0 { " checked" } else { "" },
                esc(name)
            ));
        }
        for (tk, name, html) in &panels {
            out.push_str(&format!(
                "<div class=\"panel t-{tk}\" data-target=\"{tk}\"><p class=\"panel-name\">{}</p>{html}</div>\n",
                esc(name)
            ));
        }
        out.push_str("</div>\n");
        Ok(out)
    }

    fn related(&self, keys: &[String]) -> String {
        if keys.is_empty() {
            return String::new();
        }
        let links: Vec<String> = keys
            .iter()
            .map(|k| {
                if let Some(s) = self.fw.settings.get(k) {
                    format!("<a href=\"#{}\">{}</a>", setting_id(k), esc(&s.title))
                } else if let Some(t) = self.fw.topics.get(k) {
                    format!("<a href=\"#{}\">{}</a>", topic_id(k), esc(&t.title))
                } else {
                    esc(k)
                }
            })
            .collect();
        format!("<p class=\"related\">See also: {}.</p>\n", links.join(", "))
    }

    fn values_of(&self, s: &firmware::Setting) -> Result<String> {
        Ok(match s.kind {
            SettingType::Int => {
                let unit = s.unit.as_deref().unwrap_or("");
                let mut v = format!(
                    "{} to {}",
                    fmt_num(s.min.unwrap_or(0), s.decimals, unit)?,
                    fmt_num(s.max.unwrap_or(0), s.decimals, unit)?
                );
                if let Some(step) = s.step {
                    v.push_str(&format!(
                        ", in steps of {}",
                        fmt_num(step, s.decimals, unit)?
                    ));
                }
                if let Some(z) = &s.zero_text {
                    v.push_str(&format!(". The value 0 shows as <b>{}</b>", esc(z)));
                }
                v
            }
            SettingType::Enum => s
                .options
                .iter()
                .map(|o| format!("<b>{}</b>", esc(&o.text)))
                .collect::<Vec<_>>()
                .join(", "),
            SettingType::Toggle => format!(
                "<b>{}</b> or <b>{}</b>",
                esc(s.on_text.as_deref().unwrap_or("On")),
                esc(s.off_text.as_deref().unwrap_or("Off"))
            ),
        })
    }

    fn default_of(&self, s: &firmware::Setting) -> Result<String> {
        let d = match &s.default {
            Some(d) => d,
            None => return Ok(String::new()),
        };
        Ok(match s.kind {
            SettingType::Enum => {
                let text = s
                    .options
                    .iter()
                    .find(|o| &o.value == d)
                    .map(|o| o.text.clone());
                match text {
                    Some(t) => format!("<b>{}</b>", esc(&t)),
                    None => bail!("the default {d} is not one of the options"),
                }
            }
            SettingType::Toggle => {
                let on = d.as_bool().unwrap_or(false);
                let text = if on { &s.on_text } else { &s.off_text };
                format!(
                    "<b>{}</b>",
                    esc(text.as_deref().unwrap_or(if on { "On" } else { "Off" }))
                )
            }
            SettingType::Int => {
                let n = match d.as_i64() {
                    Some(n) => n,
                    None => bail!("the default {d} is not a whole number"),
                };
                match (&s.zero_text, n) {
                    (Some(z), 0) => format!("<b>{}</b>", esc(z)),
                    _ => fmt_num(n, s.decimals, s.unit.as_deref().unwrap_or(""))?,
                }
            }
        })
    }

    fn predicate_words(&self, key: &str) -> String {
        self.fw
            .predicates
            .get(key)
            .map(|p| p.description.clone())
            .unwrap_or_else(|| format!("`{key}`"))
    }

    fn fw_media(&self, keys: &[String]) -> Result<String> {
        let mut out = String::new();
        for k in keys {
            let m = &self.fw.media[k];
            out.push_str(
                &m.html(self.loaded, self.fw_dir)
                    .with_context(|| format!("the media `{k}`"))?,
            );
        }
        Ok(out)
    }

    // ----- the menu tree -----

    fn menu_map(&self) -> String {
        let mut out = String::from("<figure class=\"tree\">\n");
        out.push_str(&tree::svg(self.fw, &self.placed));
        out.push_str(
            "<figcaption>The menu tree. Select a box to open its section.</figcaption></figure>\n",
        );
        out.push_str("<ul class=\"menu-map\">\n");
        self.map_screen(&self.fw.root, &mut out);
        out.push_str("</ul>\n");
        out
    }

    fn map_screen(&self, key: &str, out: &mut String) {
        let s = match self.fw.screens.get(key) {
            Some(s) => s,
            None => return,
        };
        out.push_str(&format!(
            "<li><a href=\"#{}\"><b>{}</b></a><ul>\n",
            tree::screen_id(key),
            esc(&s.title)
        ));
        for it in &s.items {
            match it.kind {
                ItemType::Submenu => {
                    if let Some(r) = &it.target {
                        self.map_screen(r, out);
                    }
                }
                ItemType::Back => {}
                ItemType::Setting => {
                    let k = it.key.as_deref().unwrap_or_default();
                    out.push_str(&format!(
                        "<li><a href=\"#{}\">{}</a></li>\n",
                        setting_id(k),
                        esc(&it.render)
                    ));
                }
            }
        }
        out.push_str("</ul></li>\n");
    }

    // ----- the controls and the interactions -----

    fn controls(&self) -> Result<String> {
        let mut out = String::from("<table class=\"controls\"><thead><tr><th>Control</th><th>Press</th><th>What it does</th></tr></thead><tbody>\n");
        for c in self.fw.controls.values() {
            let n = c.events.len();
            let mut first = true;
            for (ek, e) in &c.events {
                let press = match e.hold_ms {
                    Some(ms) => format!("Hold for {ms} ms or more"),
                    None => match ek.as_str() {
                        "short" => "Press".to_string(),
                        other => other.to_string(),
                    },
                };
                if first {
                    out.push_str(&format!(
                        "<tr><td rowspan=\"{n}\"><b>{}</b>{}</td><td>{}</td><td>{}</td></tr>\n",
                        esc(&c.name),
                        paras(&c.description),
                        esc(&press),
                        esc(&e.description)
                    ));
                    first = false;
                } else {
                    out.push_str(&format!(
                        "<tr><td>{}</td><td>{}</td></tr>\n",
                        esc(&press),
                        esc(&e.description)
                    ));
                }
            }
        }
        out.push_str("</tbody></table>\n");
        for c in self.fw.controls.values() {
            out.push_str(&self.fw_media(&c.media)?);
        }
        Ok(out)
    }

    fn interactions(&self) -> String {
        let mut out = String::from("<table class=\"interactions\"><thead><tr><th>Do this</th><th>Where</th><th>Result</th></tr></thead><tbody>\n");
        for i in self.fw.interactions.values() {
            let c = &self.fw.controls[&i.control];
            let e = &c.events[&i.event];
            let action = match e.hold_ms {
                Some(ms) => format!("Hold {} for {ms} ms", c.name),
                None => format!("Press {}", c.name),
            };
            let place = if i.applies_to.is_empty() {
                "Each screen that takes input".to_string()
            } else {
                i.applies_to
                    .iter()
                    .map(|p| place_words(p))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            out.push_str(&format!(
                "<tr><td><b>{}</b></td><td>{}</td><td>{}</td></tr>\n",
                esc(&action),
                esc(&place),
                esc(&i.description)
            ));
        }
        out.push_str("</tbody></table>\n");
        out
    }

    fn home(&self) -> Result<String> {
        let h = &self.fw.home;
        let mut out = paras(&h.description);
        out.push_str(&self.fw_media(&h.media)?);
        if !h.indicators.is_empty() {
            out.push_str("<table class=\"indicators\"><thead><tr><th>Mark</th><th>What it means</th></tr></thead><tbody>\n");
            for (k, text) in &h.indicators {
                out.push_str(&format!(
                    "<tr><td><b>{}</b></td><td>{}</td></tr>\n",
                    esc(&k.replace('_', " ")),
                    esc(text)
                ));
            }
            out.push_str("</tbody></table>\n");
        }
        Ok(out)
    }

    // ----- the topics -----

    /// Every topic: those of the firmware first, then those that only the app has.
    fn topics(&self) -> Result<String> {
        let mut out = String::new();
        for key in self.fw.topics.keys() {
            out.push_str(&self.topic(key)?);
        }
        if let Some(app) = self.app {
            for key in app.topics.keys() {
                if !self.fw.topics.contains_key(key) {
                    out.push_str(&self.topic(key)?);
                }
            }
        }
        Ok(out)
    }

    /// One topic. A topic that only the app has gets the tabs of the app alone.
    fn topic(&self, key: &str) -> Result<String> {
        let t = match self.fw.topics.get(key) {
            Some(t) => t,
            None => {
                let entry = match self.app.and_then(|a| a.topics.get(key)) {
                    Some(e) => e,
                    None => bail!("{{{{topic {key}}}}}: no document has such a topic"),
                };
                let id = topic_id(key);
                let mut out = format!(
                    "<section class=\"topic\" id=\"{id}\">\n<h3>{} <code class=\"key\">{}</code></h3>\n",
                    esc(&entry.title),
                    esc(key)
                );
                out.push_str(&self.tabs(&id, None, key)?);
                out.push_str("</section>\n");
                return Ok(out);
            }
        };
        let id = topic_id(key);
        let mut out = format!(
            "<section class=\"topic\" id=\"{id}\">\n<h3>{} <code class=\"key\">{}</code></h3>\n",
            esc(&t.title),
            esc(key)
        );
        let mut board = paras(&t.description);
        if !t.interactions.is_empty() {
            board.push_str("<table class=\"interactions\"><thead><tr><th>Do this</th><th>Result</th></tr></thead><tbody>\n");
            for ik in &t.interactions {
                let i = &self.fw.interactions[ik];
                let c = &self.fw.controls[&i.control];
                let e = &c.events[&i.event];
                let action = match e.hold_ms {
                    Some(ms) => format!("Hold {} for {ms} ms", c.name),
                    None => format!("Press {}", c.name),
                };
                board.push_str(&format!(
                    "<tr><td><b>{}</b></td><td>{}</td></tr>\n",
                    esc(&action),
                    esc(&i.description)
                ));
            }
            board.push_str("</tbody></table>\n");
        }
        board.push_str(&self.fw_media(&t.media)?);
        board.push_str(&self.related(&t.related));
        out.push_str(&self.tabs(&id, Some(board), key)?);
        out.push_str("</section>\n");
        Ok(out)
    }

    /// The platforms of the app, with what each one does.
    fn app_targets(&self) -> String {
        let app = match self.app {
            Some(a) => a,
            None => return "<p>This manual has no application document.</p>\n".to_string(),
        };
        let mut out = String::from("<table class=\"targets\"><thead><tr><th>Platform</th><th>What it does</th></tr></thead><tbody>\n");
        for t in app.targets.values() {
            out.push_str(&format!(
                "<tr><td><b>{}</b></td><td>{}</td></tr>\n",
                esc(&t.name),
                t.description.as_deref().map(paras).unwrap_or_default()
            ));
        }
        out.push_str("</tbody></table>\n");
        out
    }

    fn media_item(&self, key: &str) -> Result<String> {
        match self.fw.media.get(key) {
            Some(m) => m.html(self.loaded, self.fw_dir),
            None => bail!("{{{{media {key}}}}}: the firmware document has no such media item"),
        }
    }

    // ----- the board -----

    fn board_table(&self) -> String {
        let b = self.board;
        let mut out = String::from("<table class=\"board\"><tbody>\n");
        let mut row = |k: &str, v: String| {
            out.push_str(&format!("<tr><th>{}</th><td>{}</td></tr>\n", esc(k), v))
        };
        row("Board", esc(&self.loaded.manifest.board.name));
        row(
            "Outline",
            format!("{:.2} mm &times; {:.2} mm", b.width_mm, b.height_mm),
        );
        row("Firmware", esc(&self.fw.firmware.version));
        row(
            "Board models",
            self.fw
                .firmware
                .board_models
                .iter()
                .map(|m| format!("<code>{}</code>", esc(m)))
                .collect::<Vec<_>>()
                .join(", "),
        );
        for (k, v) in &b.counts {
            row(&k.replace('_', " "), v.to_string());
        }
        out.push_str("</tbody></table>\n");
        out
    }

    fn provenance_table(&self) -> String {
        let mut out = String::from("<table class=\"provenance\"><thead><tr><th>Source</th><th>Where</th><th>Version</th></tr></thead><tbody>\n");
        for (what, place, ver) in &self.provenance {
            out.push_str(&format!(
                "<tr><td>{}</td><td><code>{}</code></td><td><code>{}</code></td></tr>\n",
                esc(what),
                esc(place),
                esc(ver)
            ));
        }
        out.push_str("</tbody></table>\n");
        out
    }

    // ----- images -----

    fn image(&self, args: &str) -> Result<String> {
        let (spec, alt) = match args.split_once('|') {
            Some((s, a)) => (s.trim(), a.trim()),
            None => (args.trim(), ""),
        };
        if alt.is_empty() {
            bail!("{{{{image {spec}}}}} needs a caption after `|`");
        }
        let path = self.loaded.resolve(spec)?;
        let body = media::image_file(&path, alt)?;
        Ok(format!(
            "<figure>{body}<figcaption>{}</figcaption></figure>\n",
            esc(alt)
        ))
    }
}

fn kind_words(kind: SettingType) -> &'static str {
    match kind {
        SettingType::Int => "a number",
        SettingType::Enum => "a choice",
        SettingType::Toggle => "a switch",
    }
}

fn scope_words(scope: Scope) -> &'static str {
    match scope {
        Scope::Device => "One value for the board",
        Scope::Profile => "One value in each profile",
        Scope::Runtime => "Not stored. The board starts with the default at each start",
    }
}

fn place_words(place: &str) -> &str {
    match place {
        "home" => "The home screen",
        "screen" => "A menu screen",
        "edit_int" => "The number editor",
        "edit_enum" => "The choice editor",
        "locked" => "The locked screen",
        other => other,
    }
}

/// `105` with one decimal and `bps` reads `10.5 bps`.
pub fn fmt_num(n: i64, decimals: u32, unit: &str) -> Result<String> {
    let text = if decimals == 0 {
        n.to_string()
    } else {
        let div = match 10_u64.checked_pow(decimals) {
            Some(d) => d,
            None => bail!("{decimals} decimal places is more than a number can hold"),
        };
        let sign = if n < 0 { "-" } else { "" };
        let n = n.unsigned_abs();
        format!(
            "{sign}{}.{:0width$}",
            n / div,
            n % div,
            width = decimals as usize
        )
    };
    Ok(if unit.is_empty() {
        text
    } else {
        format!("{text}&nbsp;{}", esc(unit))
    })
}

/// Paragraphs of plain text as HTML, with `code` for a span in backticks and a link for
/// `[words](#id)`. A paragraph that starts with a number and a dot is a step.
pub fn paras(text: &str) -> String {
    // The three patterns are constants, so they compile once and cannot fail.
    static RE_CODE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"`([^`]+)`").expect("a constant pattern"));
    static RE_LINK: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\[([^\]]+)\]\((#[A-Za-z0-9_-]+)\)").expect("a constant pattern")
    });
    static RE_STEP: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(\d+)\.\s+").expect("a constant pattern"));
    let (re_code, re_link, re_step) = (&*RE_CODE, &*RE_LINK, &*RE_STEP);
    text.split("\n\n")
        .filter(|p| !p.trim().is_empty())
        .map(|p| {
            let e = esc(p.trim());
            let e = re_code.replace_all(&e, "<code>$1</code>");
            let e = re_link.replace_all(&e, "<a href=\"$2\">$1</a>");
            match re_step.captures(&e) {
                Some(c) => format!(
                    "<p class=\"step\"><span class=\"step-n\">{}</span> {}</p>",
                    &c[1],
                    &e[c[0].len()..]
                ),
                None => format!("<p>{e}</p>"),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_print_with_decimals_and_units() {
        assert_eq!(fmt_num(105, 1, "bps").unwrap(), "10.5&nbsp;bps");
        assert_eq!(fmt_num(5, 0, "").unwrap(), "5");
        assert_eq!(fmt_num(1000, 1, "ms").unwrap(), "100.0&nbsp;ms");
        assert_eq!(fmt_num(i64::MIN, 1, "").unwrap(), "-922337203685477580.8");
        assert!(fmt_num(1, 20, "").is_err());
    }

    #[test]
    fn ids_come_from_shared_keys() {
        assert_eq!(
            setting_id("autococker.eye_mode"),
            "setting-autococker-eye-mode"
        );
        assert_eq!(topic_id("topic.eye_override"), "topic-eye-override");
    }

    #[test]
    fn a_numbered_paragraph_is_a_step() {
        let html = paras("Text.\n\n1. Pull the trigger.\n\n2. Wait.");
        assert!(html
            .contains("<p class=\"step\"><span class=\"step-n\">1</span> Pull the trigger.</p>"));
        assert!(html.starts_with("<p>Text.</p>"));
    }
}

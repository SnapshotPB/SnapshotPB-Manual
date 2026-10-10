//! The blocks that the generator writes from the models: the settings reference, the button
//! tables, the enum tables, the word tables, the board figures and the provenance.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use base64::Engine;
use regex::Regex;

use crate::board::BoardInfo;
use crate::defaults::Defaults;
use crate::fsm::Fsm;
use crate::manifest::Loaded;
use crate::menu::{Item, Kind, Lang, MenuModel};
use crate::proto::{EnumDef, ProtoModel};
use crate::util::{esc, read_bytes, read_text, slug};

/// The words of the author for a setting, an option or a caption, keyed by bind, symbol or
/// `named.<key>`.
#[derive(Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Descriptions {
    #[serde(default)]
    pub text: BTreeMap<String, String>,
}

pub struct Ctx<'a> {
    pub loaded: &'a Loaded,
    pub menu: &'a MenuModel,
    pub lang: &'a Lang,
    pub proto: &'a ProtoModel,
    pub fsm: &'a Fsm,
    pub defaults: &'a Defaults,
    pub board: &'a BoardInfo,
    pub descriptions: &'a Descriptions,
    pub values: BTreeMap<String, String>,
    /// `(what, where, version)` rows.
    pub provenance: Vec<(String, String, String)>,
}

impl<'a> Ctx<'a> {
    /// Runs one `{{name args}}` directive.
    pub fn directive(&self, name: &str, args: &str) -> Result<String> {
        match name {
            "menu" => self.menu_reference(),
            "menu-map" => Ok(self.menu_map()),
            "buttons" => self.buttons(),
            "enum" => self.enum_table(args.trim()),
            "languages" => Ok(self.languages()),
            "captions" => Ok(self.captions()),
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
        if let Some(name) = key.strip_prefix("define.") {
            return match self.defaults.defines.get(name) {
                Some(v) => Ok(v.clone()),
                None => bail!("{{{{value {key}}}}}: no such define"),
            };
        }
        if let Some(name) = key.strip_prefix("kconfig.") {
            return match self.defaults.kconfig.get(name) {
                Some(v) => Ok(v.clone()),
                None => bail!("{{{{value {key}}}}}: no such Kconfig default"),
            };
        }
        bail!("{{{{value {key}}}}}: unknown value")
    }

    // ----- the settings reference -----

    fn menu_reference(&self) -> Result<String> {
        let mut out = String::new();
        for s in &self.menu.screens {
            let id = format!("screen-{}", slug(&s.key));
            out.push_str(&format!("<h3 id=\"{}\">{}</h3>\n", id, esc(&s.title)));
            let path: Vec<String> = s.path.iter().map(|p| esc(p)).collect();
            out.push_str(&format!(
                "<p class=\"crumbs\">{}</p>\n",
                path.join(" &rsaquo; ")
            ));
            out.push_str("<table class=\"settings\"><thead><tr><th>Row</th><th>Values</th><th>Default</th><th>What it does</th></tr></thead><tbody>\n");
            for it in &s.items {
                out.push_str(
                    &self
                        .row(it)
                        .with_context(|| format!("row `{}` of `{}`", it.render, s.key))?,
                );
            }
            out.push_str("</tbody></table>\n");
        }
        Ok(out)
    }

    fn row(&self, it: &Item) -> Result<String> {
        let values = match it.kind {
            Kind::Int => {
                let unit = it.unit.as_deref().unwrap_or("");
                let mut v = format!(
                    "{} to {}",
                    fmt_num(it.min.unwrap_or(0), it.decimals, unit),
                    fmt_num(it.max.unwrap_or(0), it.decimals, unit)
                );
                if let Some(step) = it.step {
                    v.push_str(&format!(
                        ", in steps of {}",
                        fmt_num(step, it.decimals, unit)
                    ));
                }
                if let Some(z) = &it.zero_text {
                    v.push_str(&format!(". The value 0 shows as <b>{}</b>", esc(z)));
                }
                v
            }
            Kind::Enum => it
                .options
                .iter()
                .map(|o| format!("<b>{}</b>", esc(&o.text)))
                .collect::<Vec<_>>()
                .join(", "),
            Kind::Toggle => "<b>On</b> or <b>Off</b>".to_string(),
            Kind::Submenu => {
                let target = it
                    .reference
                    .as_deref()
                    .and_then(|r| self.menu.screen(r))
                    .map(|s| s.title.clone())
                    .unwrap_or_default();
                format!(
                    "Opens the <a href=\"#screen-{}\">{}</a> screen",
                    slug(it.reference.as_deref().unwrap_or("")),
                    esc(&target)
                )
            }
            Kind::Back => "Goes back one screen".to_string(),
        };
        let default = self.default_of(it)?;
        let mut what = self.description_of(it);
        if let Some(p) = &it.hidden_when {
            what.push_str(&format!(
                "<p class=\"when\">The row is hidden while {}.</p>",
                esc(&self.guard_words(p))
            ));
        }
        if let Some(p) = &it.disabled_when {
            what.push_str(&format!(
                "<p class=\"when\">The row does not apply while {}.</p>",
                esc(&self.guard_words(p))
            ));
        }
        let kind = match it.kind {
            Kind::Int => "number",
            Kind::Enum => "choice",
            Kind::Toggle => "switch",
            Kind::Submenu => "screen",
            Kind::Back => "back",
        };
        Ok(format!(
            "<tr class=\"k-{kind}\"><td><b>{}</b><span class=\"kind\">{kind}</span></td><td>{values}</td><td>{default}</td><td>{what}</td></tr>\n",
            esc(&it.render)
        ))
    }

    fn default_of(&self, it: &Item) -> Result<String> {
        let bind = match &it.bind {
            Some(b) => b,
            None => return Ok(String::new()),
        };
        let spec = match self.loaded.manifest.defaults.get(bind) {
            Some(s) => s,
            None => return Ok(String::new()),
        };
        let (raw, is_enum) = self
            .defaults
            .resolve(spec)
            .with_context(|| format!("default of `{bind}`"))?;
        if is_enum || it.kind == Kind::Enum {
            let text = it
                .options
                .iter()
                .find(|o| o.value == raw || raw.ends_with(&format!("_{}", o.value)))
                .map(|o| o.text.clone())
                .unwrap_or(raw);
            return Ok(format!("<b>{}</b>", esc(&text)));
        }
        if it.kind == Kind::Toggle {
            return Ok(format!("<b>{}</b>", esc(&raw)));
        }
        let n: i64 = match raw.parse() {
            Ok(n) => n,
            Err(_) => bail!("the default `{raw}` of `{bind}` is not a number"),
        };
        if n == 0 {
            if let Some(z) = &it.zero_text {
                return Ok(format!("<b>{}</b>", esc(z)));
            }
        }
        Ok(fmt_num(n, it.decimals, it.unit.as_deref().unwrap_or("")))
    }

    /// The words of the author for a row, or the comment of the protocol field.
    fn description_of(&self, it: &Item) -> String {
        if let Some(b) = &it.bind {
            if let Some(t) = self.descriptions.text.get(b) {
                return paras(t);
            }
            if let Some((scope, field)) = b.split_once('.') {
                if let Some(msg) = self.loaded.manifest.firmware.scopes.get(scope) {
                    if let Some(f) = self.proto.field(msg, field) {
                        return paras(&f.doc);
                    }
                }
            }
        }
        if let Some(t) = self.descriptions.text.get(&it.render) {
            return paras(t);
        }
        String::new()
    }

    fn guard_words(&self, guard: &str) -> String {
        guard
            .split(',')
            .map(|g| {
                let g = g.trim();
                self.loaded
                    .manifest
                    .firmware
                    .guards
                    .get(g)
                    .cloned()
                    .unwrap_or_else(|| format!("`{g}`"))
            })
            .collect::<Vec<_>>()
            .join(" and ")
    }

    // ----- the menu map -----

    fn menu_map(&self) -> String {
        let mut out = String::from("<ul class=\"menu-map\">\n");
        self.map_screen("root", &mut out);
        out.push_str("</ul>\n");
        out
    }

    fn map_screen(&self, key: &str, out: &mut String) {
        let s = match self.menu.screen(key) {
            Some(s) => s,
            None => return,
        };
        out.push_str(&format!(
            "<li><a href=\"#screen-{}\"><b>{}</b></a><ul>\n",
            slug(key),
            esc(&s.title)
        ));
        for it in &s.items {
            match it.kind {
                Kind::Submenu => {
                    if let Some(r) = &it.reference {
                        self.map_screen(r, out);
                    }
                }
                Kind::Back => {}
                _ => out.push_str(&format!("<li>{}</li>\n", esc(&it.render))),
            }
        }
        out.push_str("</ul></li>\n");
    }

    // ----- the buttons -----

    fn buttons(&self) -> Result<String> {
        let fw = &self.loaded.manifest.firmware;
        for m in &self.fsm.modes {
            if !fw.states.iter().any(|s| &s.state == m) {
                bail!("the interaction FSM has the mode `{m}`, and the manifest gives it no words (firmware.states)");
            }
        }
        let mut out = String::new();
        for st in &fw.states {
            if !self.fsm.modes.contains(&st.state) {
                bail!(
                    "the manifest names the state `{}`, and the interaction FSM has no such mode",
                    st.state
                );
            }
            let rows: Vec<&crate::fsm::Transition> = self.fsm.from_mode(&st.state).collect();
            if rows.is_empty() {
                continue;
            }
            out.push_str(&format!(
                "<h3 id=\"mode-{}\">{}</h3>\n",
                slug(&st.state),
                esc(&st.name)
            ));
            if !st.text.is_empty() {
                out.push_str(&format!("<p>{}</p>\n", esc(&st.text)));
            }
            out.push_str("<table class=\"buttons\"><thead><tr><th>Action</th><th>Condition</th><th>Result</th></tr></thead><tbody>\n");
            for t in rows {
                let action = t
                    .events
                    .iter()
                    .map(|e| {
                        fw.events
                            .get(e)
                            .cloned()
                            .unwrap_or_else(|| format!("`{e}`"))
                    })
                    .collect::<Vec<_>>()
                    .join(", or ");
                let cond = t
                    .guard
                    .as_deref()
                    .map(|g| self.guard_words(g))
                    .unwrap_or_default();
                let mut result = String::new();
                if t.to != t.from {
                    let name = fw
                        .states
                        .iter()
                        .find(|s| s.state == t.to)
                        .map(|s| s.name.clone())
                        .unwrap_or_else(|| t.to.clone());
                    result.push_str(&format!("Goes to <b>{}</b>", esc(&name)));
                }
                if let Some(n) = &t.note {
                    let words = self
                        .descriptions
                        .text
                        .get(&format!("note.{n}"))
                        .cloned()
                        .unwrap_or_else(|| n.clone());
                    if !result.is_empty() {
                        result.push_str(". ");
                    }
                    result.push_str(&esc(&words));
                }
                if result.is_empty() {
                    result.push_str("Nothing");
                }
                out.push_str(&format!(
                    "<tr><td>{}</td><td>{}</td><td>{}</td></tr>\n",
                    esc(&action),
                    esc(&cond),
                    result
                ));
            }
            out.push_str("</tbody></table>\n");
        }
        Ok(out)
    }

    // ----- enums -----

    fn enum_table(&self, name: &str) -> Result<String> {
        let e: &EnumDef = match self.proto.enum_def(name) {
            Some(e) => e,
            None => bail!("{{{{enum {name}}}}}: the protocol has no such enum"),
        };
        let prefix = common_prefix(
            &e.values
                .iter()
                .map(|v| v.symbol.as_str())
                .collect::<Vec<_>>(),
        );
        let mut out = String::new();
        match self.descriptions.text.get(&format!("enum.{name}")) {
            Some(intro) => out.push_str(&paras(intro)),
            None => out.push_str(&paras(&e.doc)),
        }
        out.push_str("<table class=\"enum\"><thead><tr><th>Value</th><th>What it means</th></tr></thead><tbody>\n");
        for v in &e.values {
            if v.symbol.ends_with("_UNSPECIFIED") {
                continue;
            }
            let short = v.symbol.strip_prefix(&prefix).unwrap_or(&v.symbol);
            // The panel draws the option with a space in place of `_`.
            let shown = self
                .menu
                .screens
                .iter()
                .flat_map(|s| s.items.iter())
                .flat_map(|i| i.options.iter())
                .find(|o| o.value == short)
                .map(|o| o.text.clone())
                .unwrap_or_else(|| short.replace('_', " "));
            let doc = self
                .descriptions
                .text
                .get(&v.symbol)
                .map(|t| paras(t))
                .unwrap_or_else(|| paras(&v.doc));
            out.push_str(&format!(
                "<tr><td><b>{}</b></td><td>{}</td></tr>\n",
                esc(&shown),
                doc
            ));
        }
        out.push_str("</tbody></table>\n");
        Ok(out)
    }

    // ----- the words of the panel -----

    fn languages(&self) -> String {
        let codes: Vec<&str> = self
            .lang
            .languages
            .iter()
            .map(|(c, _)| c.as_str())
            .collect();
        let mut strings: Vec<String> = Vec::new();
        let mut push = |s: &str| {
            if !s.is_empty() && !strings.iter().any(|x| x == s) {
                strings.push(s.to_string());
            }
        };
        for s in &self.menu.screens {
            push(&s.title);
            for it in &s.items {
                push(&it.render);
                for o in &it.options {
                    push(&o.text);
                }
                if let Some(u) = &it.unit {
                    push(u);
                }
                if let Some(z) = &it.zero_text {
                    push(z);
                }
            }
        }
        for (_, t) in &self.lang.named {
            push(t);
        }
        let mut out = String::from("<table class=\"languages\"><thead><tr><th>EN</th>");
        for c in &codes {
            out.push_str(&format!("<th>{}</th>", esc(c)));
        }
        out.push_str("</tr></thead><tbody>\n");
        for s in &strings {
            out.push_str(&format!("<tr><td>{}</td>", esc(s)));
            for c in &codes {
                out.push_str(&format!("<td>{}</td>", esc(&self.lang.word(c, s))));
            }
            out.push_str("</tr>\n");
        }
        out.push_str("</tbody></table>\n");
        out
    }

    fn captions(&self) -> String {
        let mut out = String::from("<table class=\"captions\"><thead><tr><th>Caption</th><th>What it means</th></tr></thead><tbody>\n");
        for (key, text) in &self.lang.named {
            if let Some(words) = self.descriptions.text.get(&format!("named.{key}")) {
                out.push_str(&format!(
                    "<tr><td><b>{}</b></td><td>{}</td></tr>\n",
                    esc(text),
                    paras(words)
                ));
            }
        }
        out.push_str("</tbody></table>\n");
        out
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
        row("Firmware target", esc(&self.loaded.manifest.firmware.board));
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
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let body = match ext.as_str() {
            "svg" => inline_svg(&path, alt)?,
            "png" | "jpg" | "jpeg" | "webp" | "gif" => {
                let mime = match ext.as_str() {
                    "png" => "image/png",
                    "jpg" | "jpeg" => "image/jpeg",
                    "webp" => "image/webp",
                    _ => "image/gif",
                };
                let data = base64::engine::general_purpose::STANDARD.encode(read_bytes(&path)?);
                format!(
                    "<img src=\"data:{mime};base64,{data}\" alt=\"{}\" loading=\"lazy\">",
                    esc(alt)
                )
            }
            other => {
                bail!("{{{{image {spec}}}}}: `{other}` is not an image type that the manual embeds")
            }
        };
        Ok(format!(
            "<figure>{body}<figcaption>{}</figcaption></figure>\n",
            esc(alt)
        ))
    }
}

/// An SVG file as inline markup: no XML declaration, no comment, and an accessible name.
pub fn inline_svg(path: &Path, alt: &str) -> Result<String> {
    let text = read_text(path)?;
    let re_decl = Regex::new(r"(?s)<\?xml.*?\?>|<!--.*?-->|<!DOCTYPE[^>]*>").unwrap();
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

/// `105` with one decimal and `bps` reads `10.5 bps`.
pub fn fmt_num(n: i64, decimals: u32, unit: &str) -> String {
    let text = if decimals == 0 {
        n.to_string()
    } else {
        let div = 10_i64.pow(decimals);
        let sign = if n < 0 { "-" } else { "" };
        let n = n.abs();
        format!(
            "{sign}{}.{:0width$}",
            n / div,
            n % div,
            width = decimals as usize
        )
    };
    if unit.is_empty() {
        text
    } else {
        format!("{text}&nbsp;{}", esc(unit))
    }
}

/// Paragraphs of plain text as HTML, with `code` for a span in backticks and a link for
/// `[words](#id)`.
pub fn paras(text: &str) -> String {
    let re_code = Regex::new(r"`([^`]+)`").unwrap();
    let re_link = Regex::new(r"\[([^\]]+)\]\((#[A-Za-z0-9_-]+)\)").unwrap();
    text.split("\n\n")
        .filter(|p| !p.trim().is_empty())
        .map(|p| {
            let e = esc(p.trim());
            let e = re_code.replace_all(&e, "<code>$1</code>");
            let e = re_link.replace_all(&e, "<a href=\"$2\">$1</a>");
            format!("<p>{e}</p>")
        })
        .collect()
}

fn common_prefix(symbols: &[&str]) -> String {
    if symbols.is_empty() {
        return String::new();
    }
    let first = symbols[0];
    let mut end = first.len();
    for s in symbols {
        let common = first
            .bytes()
            .zip(s.bytes())
            .take_while(|(a, b)| a == b)
            .count();
        end = end.min(common);
    }
    // Cut at the last `_` in the common part, so that a prefix is whole words.
    match first[..end].rfind('_') {
        Some(i) => first[..=i].to_string(),
        None => String::new(),
    }
}

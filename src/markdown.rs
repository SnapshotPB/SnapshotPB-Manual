//! The chapters: Markdown with directives, rendered to HTML with heading ids and a table of
//! contents.

use std::path::Path;

use anyhow::{bail, Context, Result};
use pulldown_cmark::{html, CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use regex::Regex;

use crate::gen::Ctx;
use crate::util::{esc, read_text, slug};

/// The trademarks and the legal notice. Each manual ends with this chapter, after the
/// chapters of its content directory.
const LEGAL: &str = include_str!("../assets/legal.md");

/// The `#` heading of the legal chapter, as the table of contents holds it.
pub const LEGAL_TITLE: &str = "Trademarks and legal notice";

/// The level of a chapter in the table of contents. A `#` heading is an `h2` of the page.
pub const CHAPTER_LEVEL: u8 = 2;

pub struct TocEntry {
    pub level: u8,
    pub id: String,
    pub text: String,
}

pub struct Body {
    pub html: String,
    pub toc: Vec<TocEntry>,
}

/// Reads every `.md` file of the content directory, in name order, adds the legal chapter
/// at the end, and renders them as one body. A `#` heading of a chapter is an `h2` of the
/// page.
pub fn render_dir(dir: &Path, ctx: &Ctx) -> Result<Body> {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| format!("cannot list {}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map(|e| e == "md").unwrap_or(false))
        .collect();
    files.sort();
    if files.is_empty() {
        bail!("no .md chapter under {}", dir.display());
    }
    let mut source = String::new();
    for f in &files {
        source.push_str(&read_text(f)?);
        source.push_str("\n\n");
    }
    source.push_str(LEGAL);
    let body = render(&source, ctx)?;
    // A block that the last content file does not close takes in the legal chapter, and the
    // page then has no legal chapter. The build stops in that case.
    if let Some(last) = files.last() {
        legal_entry(&body.toc).with_context(|| format!("after {}", last.display()))?;
    }
    Ok(body)
}

/// The entry of the legal chapter in the table of contents. It is the last chapter of the
/// page, and it has the title of the legal chapter. A different last chapter is an error.
pub fn legal_entry(toc: &[TocEntry]) -> Result<&TocEntry> {
    match toc.iter().rev().find(|e| e.level == CHAPTER_LEVEL) {
        Some(e) if e.text == LEGAL_TITLE => Ok(e),
        _ => bail!(
            "the page does not end with the chapter `{LEGAL_TITLE}`: close each HTML comment and each code fence of the last chapter"
        ),
    }
}

pub fn render(source: &str, ctx: &Ctx) -> Result<Body> {
    // Pass 1: inline values and block directives.
    let re_value = Regex::new(r"\{\{\s*value\s+([^}\s]+)\s*\}\}").unwrap();
    let re_block = Regex::new(r"^\{\{\s*([a-z-]+)(?:\s+(.*?))?\s*\}\}$").unwrap();
    let mut blocks: Vec<String> = Vec::new();
    let mut text = String::new();
    for (n, line) in source.lines().enumerate() {
        let t = line.trim();
        if let Some(cap) = re_block.captures(t) {
            if &cap[1] != "value" {
                let html = ctx
                    .directive(&cap[1], cap.get(2).map(|m| m.as_str()).unwrap_or(""))
                    .with_context(|| format!("line {}: {t}", n + 1))?;
                blocks.push(html);
                text.push_str(&format!("\n<!--GEN:{}-->\n", blocks.len() - 1));
                continue;
            }
        }
        let mut err = None;
        let replaced =
            re_value.replace_all(line, |cap: &regex::Captures| match ctx.value(&cap[1]) {
                Ok(v) => v,
                Err(e) => {
                    err = Some(e);
                    String::new()
                }
            });
        if let Some(e) = err {
            return Err(e).with_context(|| format!("line {}", n + 1));
        }
        text.push_str(&replaced);
        text.push('\n');
    }

    // Pass 2: Markdown to HTML, with ids on the headings and admonitions on the paragraphs.
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    let events: Vec<Event> = Parser::new_ext(&text, opts).collect();
    let mut out_events: Vec<Event> = Vec::with_capacity(events.len() + 64);
    let mut toc = Vec::new();
    let mut ids: Vec<String> = Vec::new();
    let mut i = 0;
    while i < events.len() {
        match &events[i] {
            Event::Start(Tag::Heading { level, .. }) => {
                let level = shift(*level);
                // Collect the text of the heading to make its id.
                let mut j = i + 1;
                let mut title = String::new();
                while j < events.len() && !matches!(events[j], Event::End(TagEnd::Heading(_))) {
                    if let Event::Text(t) | Event::Code(t) = &events[j] {
                        title.push_str(t);
                    }
                    j += 1;
                }
                let mut id = slug(&title);
                let mut k = 2;
                while ids.contains(&id) {
                    id = format!("{}-{}", slug(&title), k);
                    k += 1;
                }
                ids.push(id.clone());
                if level <= 3 {
                    toc.push(TocEntry {
                        level,
                        id: id.clone(),
                        text: title.clone(),
                    });
                }
                out_events.push(Event::Html(CowStr::from(format!("<h{level} id=\"{id}\">"))));
                for e in &events[i + 1..j] {
                    out_events.push(e.clone());
                }
                out_events.push(Event::Html(CowStr::from(format!("</h{level}>\n"))));
                i = j + 1;
                continue;
            }
            Event::Start(Tag::Paragraph) => {
                // An admonition is a paragraph that starts with WARNING:, CAUTION: or NOTE:.
                let kind = match events.get(i + 1) {
                    Some(Event::Text(t)) => admonition(t),
                    Some(Event::Start(Tag::Strong)) => match events.get(i + 2) {
                        Some(Event::Text(t)) => admonition(t),
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(kind) = kind {
                    let mut j = i + 1;
                    while j < events.len() && !matches!(events[j], Event::End(TagEnd::Paragraph)) {
                        j += 1;
                    }
                    out_events.push(Event::Html(CowStr::from(format!(
                        "<div class=\"adm adm-{kind}\" role=\"note\"><p>"
                    ))));
                    for e in &events[i + 1..j] {
                        out_events.push(e.clone());
                    }
                    out_events.push(Event::Html(CowStr::from("</p></div>\n")));
                    i = j + 1;
                    continue;
                }
                out_events.push(events[i].clone());
            }
            e => out_events.push(e.clone()),
        }
        i += 1;
    }
    let mut html_out = String::new();
    html::push_html(&mut html_out, out_events.into_iter());

    // Pass 3: put the generated blocks back, and collect their headings into the contents.
    let re_gen = Regex::new(r"<!--GEN:(\d+)-->").unwrap();
    let html_out = re_gen.replace_all(&html_out, |cap: &regex::Captures| {
        let n: usize = cap[1].parse().unwrap_or(0);
        blocks.get(n).cloned().unwrap_or_default()
    });
    let html_out = html_out.into_owned();
    let re_h = Regex::new(r#"<h([2-4]) id="([^"]+)">(.*?)</h[2-4]>"#).unwrap();
    let re_tag = Regex::new(r"<[^>]+>").unwrap();
    let mut toc_all = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for cap in re_h.captures_iter(&html_out) {
        let level: u8 = cap[1].parse().unwrap_or(2);
        let id = cap[2].to_string();
        if seen.contains(&id) {
            bail!("two headings share the id `{id}`");
        }
        seen.push(id.clone());
        if level <= 3 {
            let raw = re_tag.replace_all(&cap[3], "").to_string();
            toc_all.push(TocEntry {
                level,
                id,
                text: unescape(&raw),
            });
        }
    }
    let _ = toc;
    // Every link into the page must reach a heading or a generated id.
    let re_id = Regex::new(r#" id="([^"]+)""#).unwrap();
    let all_ids: Vec<&str> = re_id
        .captures_iter(&html_out)
        .map(|c| c.get(1).unwrap().as_str())
        .collect();
    let re_href = Regex::new(r##"href="#([^"]+)""##).unwrap();
    for cap in re_href.captures_iter(&html_out) {
        let target = &cap[1];
        if target != "top" && target != "content" && !all_ids.contains(&target) {
            bail!("a link points to `#{target}`, and the page has no such id");
        }
    }
    Ok(Body {
        html: html_out,
        toc: toc_all,
    })
}

fn shift(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => CHAPTER_LEVEL,
        HeadingLevel::H2 => 3,
        HeadingLevel::H3 => 4,
        HeadingLevel::H4 => 5,
        _ => 6,
    }
}

fn admonition(text: &str) -> Option<&'static str> {
    let t = text.trim_start();
    if t.starts_with("WARNING:") {
        Some("warning")
    } else if t.starts_with("CAUTION:") {
        Some("caution")
    } else if t.starts_with("NOTE:") {
        Some("note")
    } else {
        None
    }
}

fn unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

/// The navigation list of the page.
pub fn toc_html(toc: &[TocEntry]) -> String {
    let mut out = String::from("<ol class=\"toc\">\n");
    let mut open3 = false;
    for e in toc {
        match e.level {
            2 => {
                if open3 {
                    out.push_str("</ol></li>\n");
                    open3 = false;
                } else if out.len() > 20 {
                    out.push_str("</li>\n");
                }
                out.push_str(&format!("<li><a href=\"#{}\">{}</a>", e.id, esc(&e.text)));
            }
            _ => {
                if !open3 {
                    out.push_str("<ol>\n");
                    open3 = true;
                }
                out.push_str(&format!(
                    "<li><a href=\"#{}\">{}</a></li>\n",
                    e.id,
                    esc(&e.text)
                ));
            }
        }
    }
    if open3 {
        out.push_str("</ol></li>\n");
    } else if out.len() > 20 {
        out.push_str("</li>\n");
    }
    out.push_str("</ol>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_legal_chapter_is_one_chapter() {
        let heading = format!("# {LEGAL_TITLE}");
        assert_eq!(LEGAL.lines().next(), Some(heading.as_str()));
        let chapters: Vec<_> = LEGAL.lines().filter(|l| l.starts_with("# ")).collect();
        assert_eq!(chapters, [heading.as_str()]);
    }

    fn entry(level: u8, id: &str, text: &str) -> TocEntry {
        TocEntry {
            level,
            id: id.into(),
            text: text.into(),
        }
    }

    #[test]
    fn the_legal_entry_is_the_last_chapter() {
        let toc = [
            entry(2, "about", "About"),
            entry(2, "trademarks-and-legal-notice", LEGAL_TITLE),
            entry(3, "safety", "Safety"),
        ];
        let e = legal_entry(&toc).expect("the legal chapter");
        assert_eq!(e.id, "trademarks-and-legal-notice");
        // A content chapter with the same title moves the id of the legal chapter.
        let toc = [
            entry(2, "trademarks-and-legal-notice", LEGAL_TITLE),
            entry(2, "trademarks-and-legal-notice-2", LEGAL_TITLE),
        ];
        assert_eq!(
            legal_entry(&toc).expect("the legal chapter").id,
            "trademarks-and-legal-notice-2"
        );
        // An open block in the last content file takes in the legal chapter.
        let toc = [entry(2, "about", "About"), entry(2, "last", "Last")];
        let err = match legal_entry(&toc) {
            Ok(e) => panic!("no legal chapter, but got `{}`", e.id),
            Err(e) => e.to_string(),
        };
        assert!(err.contains("code fence"), "{err}");
        assert!(legal_entry(&[]).is_err());
    }
}

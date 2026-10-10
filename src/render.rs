//! The page: one HTML file with every style, script and picture inside it.

use anyhow::Result;
use base64::Engine;

use crate::gen::{Ctx, RuntimeChip};
use crate::markdown::{toc_html, Body};
use crate::media::inline_svg;
use crate::theme::Theme;
use crate::util::{esc, read_bytes};

const CSS: &str = include_str!("../assets/manual.css");
const JS: &str = include_str!("../assets/manual.js");

/// The runtime selector: one chip for the board and one for each runtime of the app, in
/// groups. With no script, the chips do nothing and every block shows.
pub fn runtime_bar(chips: &[RuntimeChip]) -> String {
    if chips.len() <= 1 {
        return String::new();
    }
    let mut out = String::from(
        "<div class=\"runtime-bar\" id=\"runtime-bar\" role=\"group\" aria-label=\"The runtimes to show\">\n<span class=\"runtime-label\">Show</span>\n",
    );
    let mut group = String::from("\u{0}");
    for c in chips {
        if c.group != group {
            if group != "\u{0}" {
                out.push_str("</span>\n");
            }
            group = c.group.clone();
            out.push_str(&format!(
                "<span class=\"chip-group\" data-group=\"{}\">{}",
                esc(&c.group),
                if c.group_name.is_empty() {
                    String::new()
                } else {
                    format!("<span class=\"group-name\">{}</span>", esc(&c.group_name))
                }
            ));
        }
        out.push_str(&format!(
            "<label class=\"chip\"><input type=\"checkbox\" data-runtime=\"{}\" checked> {}</label>\n",
            esc(&c.key),
            esc(&c.name)
        ));
    }
    if group != "\u{0}" {
        out.push_str("</span>\n");
    }
    out.push_str(
        "<button type=\"button\" class=\"chip-all\" id=\"runtime-all\">All</button>\n</div>\n",
    );
    out
}

pub fn page(ctx: &Ctx, theme: &Theme, body: &Body) -> Result<String> {
    let m = &ctx.loaded.manifest;
    // The full logo heads the manual. The small icon marks the top bar, where the full logo
    // would be too small to read, and it is the icon of the browser tab.
    let logo = inline_svg(
        &ctx.loaded.resolve(&m.brand.logo)?,
        &format!("{} logo", m.title),
    )?
    .replacen("<svg", "<svg class=\"hero-logo\"", 1);
    let icon_path = ctx.loaded.resolve(&m.brand.icon)?;
    let mark = inline_svg(&icon_path, &format!("{} mark", m.title))?.replacen(
        "<svg",
        "<svg class=\"logo\"",
        1,
    );
    let icon = base64::engine::general_purpose::STANDARD.encode(read_bytes(&icon_path)?);
    let version = ctx
        .values
        .get("firmware.version")
        .cloned()
        .unwrap_or_default();
    let app_version = ctx
        .values
        .get("app.version")
        .map(|v| format!(", app <code>{}</code>", esc(v)))
        .unwrap_or_default();
    let date = ctx.values.get("manual.date").cloned().unwrap_or_default();
    let title = esc(&m.title);
    let subtitle = esc(&m.subtitle);
    let toc = toc_html(&body.toc);
    let theme_css = theme.css();
    let runtime_bar = runtime_bar(&ctx.runtimes());
    let html_class = if runtime_bar.is_empty() {
        ""
    } else {
        " class=\"has-runtime-bar\""
    };
    let content = &body.html;
    let board = esc(&m.board.name);
    let md5 = esc(ctx.board.md5_short());
    let generator = env!("CARGO_PKG_NAME");
    let gen_version = env!("CARGO_PKG_VERSION");
    Ok(format!(
        r##"<!DOCTYPE html>
<html lang="en"{html_class}>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} — {subtitle}</title>
<meta name="description" content="{title} {subtitle}: the {board} board at {md5} and the firmware {version_attr}.">
<meta name="generator" content="{generator} {gen_version}">
<link rel="icon" type="image/svg+xml" href="data:image/svg+xml;base64,{icon}">
<style>
{theme_css}
{CSS}
</style>
</head>
<body>
<a class="skip" href="#content">Skip to the content</a>
<header class="top">
  <button class="iconbtn" id="navbtn" type="button" aria-label="Open the contents" aria-expanded="false" aria-controls="nav">
    <svg viewBox="0 0 24 24" width="24" height="24" aria-hidden="true"><path fill="currentColor" d="M3 6h18v2H3zm0 5h18v2H3zm0 5h18v2H3z"/></svg>
  </button>
  <a class="brand" href="#top">{mark}<span class="brand-text"><span class="brand-title">{title}</span><span class="brand-sub">{subtitle}</span></span></a>
  <button class="iconbtn" id="themebtn" type="button" aria-label="Change the colour theme" title="Theme: follows the system">
    <svg viewBox="0 0 24 24" width="24" height="24" aria-hidden="true"><path fill="currentColor" d="M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zm0 18V4a8 8 0 0 1 0 16z"/></svg>
  </button>
</header>
{runtime_bar}<div class="shell" id="top">
  <nav class="side" id="nav" aria-label="Contents">
    <p class="side-title">Contents</p>
    {toc}
  </nav>
  <div class="scrim" id="scrim" hidden></div>
  <main id="content">
    <article>
<header class="hero">
  {logo}
  <h1 class="hero-title">{title}</h1>
  <p class="hero-sub">{subtitle}</p>
</header>
{content}
    </article>
    <footer class="foot">
      <p>{title} &middot; {subtitle}. The {board} board at <code>{md5}</code>, firmware <code>{version}</code>{app_version}. Generated on {date} by <code>{generator} {gen_version}</code>.</p>
    </footer>
  </main>
</div>
<script>
{JS}
</script>
</body>
</html>
"##,
        version_attr = esc(&version),
        version = esc(&version),
        date = esc(&date),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_groups_the_chips() {
        let chips = vec![
            RuntimeChip {
                key: "board".into(),
                name: "Board".into(),
                group: String::new(),
                group_name: String::new(),
            },
            RuntimeChip {
                key: "android".into(),
                name: "Android".into(),
                group: "shared".into(),
                group_name: "Shared".into(),
            },
            RuntimeChip {
                key: "ios".into(),
                name: "iOS".into(),
                group: "shared".into(),
                group_name: "Shared".into(),
            },
            RuntimeChip {
                key: "wear".into(),
                name: "Wear OS".into(),
                group: "watch".into(),
                group_name: "Watch".into(),
            },
        ];
        let html = runtime_bar(&chips);
        assert_eq!(html.matches("<span class=\"chip-group\"").count(), 3);
        assert_eq!(html.matches("data-runtime=").count(), 4);
        assert!(html.contains("<span class=\"group-name\">Shared</span>"));
    }

    #[test]
    fn no_bar_for_the_board_alone() {
        let chips = vec![RuntimeChip {
            key: "board".into(),
            name: "Board".into(),
            group: String::new(),
            group_name: String::new(),
        }];
        assert!(runtime_bar(&chips).is_empty());
    }
}

//! The page: one HTML file with every style, script and picture inside it.

use anyhow::Result;
use base64::Engine;

use crate::gen::Ctx;
use crate::manifest::Loaded;
use crate::markdown::{toc_html, Body};
use crate::media::inline_svg;
use crate::names::Names;
use crate::theme::Theme;
use crate::util::{esc, read_bytes};

const CSS: &str = include_str!("../assets/manual.css");
const JS: &str = include_str!("../assets/manual.js");

/// The style rules of the tab groups. A page holds one rule for each target, so a group
/// shows the panel of the target whose radio button is checked, with no script.
pub fn tabs_css(targets: &[(String, String)]) -> String {
    let mut css = String::new();
    for (key, _) in targets {
        css.push_str(&format!(
            ".tabs > input.t-{key}:checked ~ .panel.t-{key} {{ display: block; }}\n"
        ));
    }
    css
}

/// The escaped texts that name the manual, in each place of the page that prints them.
struct Titles {
    title: String,
    /// The title of the browser tab.
    tab: String,
    /// The start of the meta description.
    description: String,
    /// The subtitle in the top bar, as an element, or nothing.
    brand_sub: String,
    /// The subtitle under the hero title, as an element, or nothing.
    hero_sub: String,
    /// The start of the foot line.
    foot: String,
}

/// A manifest with no subtitle gets the title alone in each place that names the manual.
fn titles(title: &str, subtitle: &str) -> Titles {
    let title = esc(title);
    let subtitle = esc(subtitle.trim());
    if subtitle.is_empty() {
        return Titles {
            tab: title.clone(),
            description: title.clone(),
            brand_sub: String::new(),
            hero_sub: String::new(),
            foot: title.clone(),
            title,
        };
    }
    Titles {
        tab: format!("{title} — {subtitle}"),
        description: format!("{title} {subtitle}"),
        brand_sub: format!("<span class=\"brand-sub\">{subtitle}</span>"),
        hero_sub: format!("\n  <p class=\"hero-sub\">{subtitle}</p>"),
        foot: format!("{title} &middot; {subtitle}"),
        title,
    }
}

/// The logo of the manual and the emblem of the board family, as inline SVG with the
/// class `hero-logo` on each. The index of the site shows the same pair.
pub fn hero_logos(loaded: &Loaded) -> Result<String> {
    let m = &loaded.manifest;
    let hero = |spec: &str, alt: &str| -> Result<String> {
        Ok(
            inline_svg(&loaded.resolve(spec)?, &format!("{} {alt}", m.title))?.replacen(
                "<svg",
                "<svg class=\"hero-logo\"",
                1,
            ),
        )
    };
    let mut logos = hero(&m.brand.logo, "logo")?;
    if let Some(emblem) = &m.brand.emblem {
        logos.push('\n');
        logos.push_str(&hero(emblem, "emblem")?);
    }
    Ok(logos)
}

pub fn page(ctx: &Ctx, theme: &Theme, body: &Body) -> Result<String> {
    let m = &ctx.loaded.manifest;
    // The full logo heads the manual, with the emblem of the board family beside it when the
    // manifest names one. The small icon marks the top bar, where the full logo would be too
    // small to read, and it is the icon of the browser tab.
    let logos = hero_logos(ctx.loaded)?;
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
    let Titles {
        title,
        tab,
        description,
        brand_sub,
        hero_sub,
        foot,
    } = titles(&m.title, &m.subtitle);
    let toc = toc_html(&body.toc);
    let theme_css = theme.css();
    let tabs_css = tabs_css(&ctx.targets());
    let content = Names::new(&m.names)?.apply(&body.html);
    let lang = esc(&ctx.lang);
    let board = esc(&m.board.name);
    let md5 = esc(ctx.board.md5_short());
    let generator = env!("CARGO_PKG_NAME");
    let gen_version = env!("CARGO_PKG_VERSION");
    Ok(format!(
        r##"<!DOCTYPE html>
<html lang="{lang}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{tab}</title>
<meta name="description" content="{description}: the {board} board at {md5} and the firmware {version_attr}.">
<meta name="generator" content="{generator} {gen_version}">
<link rel="icon" type="image/svg+xml" href="data:image/svg+xml;base64,{icon}">
<style>
{theme_css}
{CSS}
{tabs_css}
</style>
</head>
<body>
<a class="skip" href="#content">Skip to the content</a>
<header class="top">
  <button class="iconbtn" id="navbtn" type="button" aria-label="Open the contents" aria-expanded="false" aria-controls="nav">
    <svg viewBox="0 0 24 24" width="24" height="24" aria-hidden="true"><path fill="currentColor" d="M3 6h18v2H3zm0 5h18v2H3zm0 5h18v2H3z"/></svg>
  </button>
  <a class="brand" href="#top">{mark}<span class="brand-text"><span class="brand-title">{title}</span>{brand_sub}</span></a>
  <button class="iconbtn" id="themebtn" type="button" aria-label="Change the colour theme" title="Theme: follows the system">
    <svg viewBox="0 0 24 24" width="24" height="24" aria-hidden="true"><path fill="currentColor" d="M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zm0 18V4a8 8 0 0 1 0 16z"/></svg>
  </button>
</header>
<div class="shell" id="top">
  <nav class="side" id="nav" aria-label="Contents">
    <p class="side-title">Contents</p>
    {toc}
  </nav>
  <div class="scrim" id="scrim" hidden></div>
  <main id="content">
    <article>
<header class="hero">
  <div class="hero-logos">
  {logos}
  </div>
  <h1 class="hero-title">{title}</h1>{hero_sub}
</header>
{content}
    </article>
    <footer class="foot">
      <p>{foot}. The {board} board at <code>{md5}</code>, firmware <code>{version}</code>{app_version}. Generated on {date} by <code>{generator} {gen_version}</code>.</p>
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
    fn one_rule_per_target() {
        let css = tabs_css(&[
            ("board".into(), "Board".into()),
            ("android".into(), "Android".into()),
        ]);
        assert!(css.contains(".tabs > input.t-board:checked ~ .panel.t-board"));
        assert!(css.contains(".tabs > input.t-android:checked ~ .panel.t-android"));
        assert_eq!(css.lines().count(), 2);
    }

    #[test]
    fn an_empty_subtitle_leaves_the_title_alone() {
        let t = titles("SnapshotPB Autococker Manual", "  ");
        assert_eq!(t.tab, "SnapshotPB Autococker Manual");
        assert_eq!(t.description, t.tab);
        assert_eq!(t.foot, t.tab);
        assert!(t.brand_sub.is_empty());
        assert!(t.hero_sub.is_empty());
    }

    #[test]
    fn a_subtitle_follows_the_title_in_each_place() {
        let t = titles("SnapshotPB <Autococker>", "Manual");
        assert_eq!(t.title, "SnapshotPB &lt;Autococker&gt;");
        assert_eq!(t.tab, "SnapshotPB &lt;Autococker&gt; — Manual");
        assert_eq!(t.description, "SnapshotPB &lt;Autococker&gt; Manual");
        assert_eq!(t.brand_sub, "<span class=\"brand-sub\">Manual</span>");
        assert!(t.hero_sub.contains("<p class=\"hero-sub\">Manual</p>"));
        assert_eq!(t.foot, "SnapshotPB &lt;Autococker&gt; &middot; Manual");
    }
}

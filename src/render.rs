//! The page: one HTML file with every style, script and picture inside it.

use std::collections::BTreeMap;

use anyhow::{bail, Result};
use base64::Engine;

use crate::gen::{Ctx, Product};
use crate::manifest::{Loaded, Manifest};
use crate::markdown::{legal_entry, toc_html, Body};
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

/// The name of the manual in the browser tab and in a link preview, not escaped.
fn tab_title(title: &str, subtitle: &str) -> String {
    match subtitle.trim() {
        "" => title.to_string(),
        sub => format!("{title} — {sub}"),
    }
}

/// A manifest with no subtitle gets the title alone in each place that names the manual.
fn titles(title: &str, subtitle: &str) -> Titles {
    let tab = esc(&tab_title(title, subtitle));
    let title = esc(title);
    let subtitle = esc(subtitle.trim());
    if subtitle.is_empty() {
        return Titles {
            tab,
            description: title.clone(),
            brand_sub: String::new(),
            hero_sub: String::new(),
            foot: title.clone(),
            title,
        };
    }
    Titles {
        tab,
        description: format!("{title} {subtitle}"),
        brand_sub: format!("<span class=\"brand-sub\">{subtitle}</span>"),
        hero_sub: format!("\n  <p class=\"hero-sub\">{subtitle}</p>"),
        foot: format!("{title} &middot; {subtitle}"),
        title,
    }
}

/// The logo of the manual and the emblem of the board family, as inline SVG with the
/// class `hero-logo` on each. The index of the site shows the same pair. With an emblem for
/// the dark theme, both emblems go in, and the stylesheet shows the one that the theme
/// selects.
pub fn hero_logos(loaded: &Loaded) -> Result<String> {
    let m = &loaded.manifest;
    let hero = |spec: &str, alt: &str, class: &str| -> Result<String> {
        Ok(
            inline_svg(&loaded.resolve(spec)?, &format!("{} {alt}", m.title))?.replacen(
                "<svg",
                &format!("<svg class=\"{class}\""),
                1,
            ),
        )
    };
    let mut logos = hero(&m.brand.logo, "logo", "hero-logo")?;
    match (&m.brand.emblem, &m.brand.emblem_dark) {
        (Some(emblem), None) => {
            logos.push('\n');
            logos.push_str(&hero(emblem, "emblem", "hero-logo")?);
        }
        (Some(emblem), Some(dark)) => {
            logos.push('\n');
            logos.push_str(&hero(emblem, "emblem", "hero-logo theme-light")?);
            logos.push('\n');
            logos.push_str(&hero(dark, "emblem", "hero-logo theme-dark")?);
        }
        (None, Some(_)) => bail!("the manifest names `emblem_dark` and no `emblem`"),
        (None, None) => {}
    }
    Ok(logos)
}

/// The text of a link preview, not escaped.
pub struct Preview {
    pub title: String,
    pub description: String,
    /// The address of the page.
    pub url: String,
    /// The address of the picture.
    pub image: String,
}

/// The tags of a link preview: Open Graph for most sites and chat apps, Twitter for X, and
/// the canonical address. The website of SnapshotPB uses the same set.
pub fn preview_meta(p: &Preview) -> String {
    let title = esc(&p.title);
    let description = esc(&p.description);
    let url = esc(&p.url);
    let image = esc(&p.image);
    format!(
        r#"<meta property="og:type" content="website">
<meta property="og:site_name" content="SnapshotPB">
<meta property="og:title" content="{title}">
<meta property="og:description" content="{description}">
<meta property="og:image" content="{image}">
<meta property="og:url" content="{url}">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:title" content="{title}">
<meta name="twitter:description" content="{description}">
<meta name="twitter:image" content="{image}">
<link rel="canonical" href="{url}">
"#
    )
}

/// The link preview of the manual of `m` in the language `lang`, or `None` when the
/// manifest has no `[social]` table.
pub fn manual_preview(m: &Manifest, lang: &str) -> Option<Preview> {
    m.social.as_ref().map(|s| Preview {
        title: tab_title(&m.title, &m.subtitle),
        description: s.description.clone(),
        url: s.page_url(&m.id, lang),
        image: s.image.clone(),
    })
}

/// The colour of the browser bar and of the edge of a link preview: the background of
/// the app, light and dark.
pub fn theme_color_meta(theme: &Theme) -> String {
    let background = |pairs: &[(String, String)]| {
        pairs
            .iter()
            .find(|(n, _)| n == "background")
            .map(|(_, v)| esc(v))
    };
    let mut meta = String::new();
    if let Some(light) = background(&theme.light) {
        meta.push_str(&format!(
            "<meta name=\"theme-color\" media=\"(prefers-color-scheme: light)\" content=\"{light}\">\n"
        ));
    }
    if let Some(dark) = background(&theme.dark) {
        meta.push_str(&format!(
            "<meta name=\"theme-color\" media=\"(prefers-color-scheme: dark)\" content=\"{dark}\">\n"
        ));
    }
    meta
}

/// The link in the foot to the legal chapter, which is the last chapter of each body.
fn legal_link(body: &Body) -> Result<String> {
    let e = legal_entry(&body.toc)?;
    Ok(format!(
        " <a href=\"#{}\">{}</a>.",
        esc(&e.id),
        esc(&e.text)
    ))
}

/// The words that name the product and its versions: plain escaped text for the meta
/// description, and HTML for the foot.
fn product_words(product: &Product, values: &BTreeMap<String, String>) -> (String, String) {
    match *product {
        Product::Board { info, name } => {
            let name = esc(name);
            let md5 = esc(info.md5_short());
            let version = esc(values.get("firmware.version").map_or("", String::as_str));
            let app = values
                .get("app.version")
                .map(|v| format!(", app <code>{}</code>", esc(v)))
                .unwrap_or_default();
            (
                format!("the {name} board at {md5} and the firmware {version}"),
                format!(
                    "The {name} board at <code>{md5}</code>, firmware <code>{version}</code>{app}"
                ),
            )
        }
        Product::Assembly { info, name } => {
            let name = esc(name);
            let md5 = esc(info.md5_short());
            (
                format!("the {name} assembly at {md5}"),
                format!("The {name} assembly at <code>{md5}</code>"),
            )
        }
    }
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
    let (about, about_foot) = product_words(&ctx.product, &ctx.values);
    let date = ctx.values.get("manual.date").cloned().unwrap_or_default();
    let year = ctx.values.get("manual.year").cloned().unwrap_or_default();
    let legal = legal_link(body)?;
    let Titles {
        title,
        tab,
        description,
        brand_sub,
        hero_sub,
        foot,
    } = titles(&m.title, &m.subtitle);
    let preview = manual_preview(m, &ctx.lang)
        .map(|p| preview_meta(&p))
        .unwrap_or_default();
    let theme_color = theme_color_meta(theme);
    let toc = toc_html(&body.toc);
    let theme_css = theme.css();
    let tabs_css = tabs_css(&ctx.targets());
    let content = Names::new(&m.names)?.apply(&body.html);
    let lang = esc(&ctx.lang);
    let generator = env!("CARGO_PKG_NAME");
    let gen_version = env!("CARGO_PKG_VERSION");
    Ok(format!(
        r##"<!DOCTYPE html>
<html lang="{lang}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{tab}</title>
<meta name="description" content="{description}: {about}.">
<meta name="generator" content="{generator} {gen_version}">
{preview}{theme_color}<link rel="icon" type="image/svg+xml" href="data:image/svg+xml;base64,{icon}">
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
      <p>Copyright &copy; {year} SnapshotPB LLC.{legal}</p>
      <p>{foot}. {about_foot}. Generated on {date} by <code>{generator} {gen_version}</code>.</p>
    </footer>
  </main>
</div>
<script>
{JS}
</script>
</body>
</html>
"##,
        date = esc(&date),
        year = esc(&year),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dark_emblem_goes_in_beside_the_light_one() {
        let dir =
            std::env::temp_dir().join(format!("snapshotpb-manual-emblem-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a temporary directory");
        for name in ["l.svg", "e.svg", "d.svg"] {
            std::fs::write(
                dir.join(name),
                "<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
            )
            .expect("an SVG file");
        }
        let mut loaded = Loaded::empty(&dir);
        loaded.manifest.brand.logo = dir.join("l.svg").display().to_string();
        loaded.manifest.brand.emblem = Some(dir.join("e.svg").display().to_string());
        let one = hero_logos(&loaded);
        loaded.manifest.brand.emblem_dark = Some(dir.join("d.svg").display().to_string());
        let two = hero_logos(&loaded);
        loaded.manifest.brand.emblem = None;
        let none = hero_logos(&loaded);
        std::fs::remove_dir_all(&dir).expect("the temporary directory goes");
        let one = one.expect("the pair");
        assert_eq!(one.matches("<svg class=\"hero-logo\"").count(), 2, "{one}");
        let two = two.expect("the three");
        assert!(
            two.contains("<svg class=\"hero-logo theme-light\""),
            "{two}"
        );
        assert!(two.contains("<svg class=\"hero-logo theme-dark\""), "{two}");
        let err = format!("{:#}", none.expect_err("a dark emblem alone is refused"));
        assert!(err.contains("no `emblem`"), "{err}");
    }

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

    #[test]
    fn a_preview_escapes_its_text() {
        let meta = preview_meta(&Preview {
            title: "Board \"A\" & B".into(),
            description: "<b>Text</b>".into(),
            url: "https://example.com/a/en/".into(),
            image: "https://example.com/s.png".into(),
        });
        assert!(
            meta.contains(r#"<meta property="og:title" content="Board &quot;A&quot; &amp; B">"#)
        );
        assert!(
            meta.contains(r#"<meta name="twitter:description" content="&lt;b&gt;Text&lt;/b&gt;">"#)
        );
        assert!(meta.contains(r#"<link rel="canonical" href="https://example.com/a/en/">"#));
        assert!(meta.contains(r#"<meta name="twitter:card" content="summary_large_image">"#));
    }

    #[test]
    fn the_theme_colour_is_the_background_of_each_scheme() {
        let theme = Theme {
            light: vec![("background".into(), "#fafafa".into())],
            dark: vec![("primary".into(), "#ffffff".into())],
        };
        let meta = theme_color_meta(&theme);
        assert_eq!(
            meta,
            "<meta name=\"theme-color\" media=\"(prefers-color-scheme: light)\" content=\"#fafafa\">\n"
        );
    }

    #[test]
    fn the_manual_preview_names_the_page_of_the_language() {
        let mut loaded = Loaded::empty(std::path::Path::new("m.toml"));
        loaded.manifest.id = "rev-b".into();
        loaded.manifest.title = "Title".into();
        loaded.manifest.subtitle = "Sub".into();
        assert!(manual_preview(&loaded.manifest, "de").is_none());
        loaded.manifest.social = Some(crate::manifest::Social {
            site_url: "https://example.com/manuals/".into(),
            description: "A manual.".into(),
            image: "https://example.com/s.png".into(),
        });
        let p = manual_preview(&loaded.manifest, "de").expect("a preview");
        assert_eq!(p.url, "https://example.com/manuals/rev-b/de/");
        assert_eq!(p.title, "Title — Sub");
        assert_eq!(p.description, "A manual.");
        assert_eq!(p.image, "https://example.com/s.png");
        let meta = preview_meta(&p);
        assert!(meta.contains(
            r#"<meta property="og:url" content="https://example.com/manuals/rev-b/de/">"#
        ));
    }

    #[test]
    fn the_tab_title_has_the_subtitle_after_a_dash() {
        assert_eq!(tab_title("A", " B "), "A — B");
        assert_eq!(tab_title("A", ""), "A");
    }

    #[test]
    fn the_legal_link_names_the_last_chapter() {
        let entry = |level: u8, id: &str, text: &str| crate::markdown::TocEntry {
            level,
            id: id.into(),
            text: text.into(),
        };
        let body = Body {
            html: String::new(),
            toc: vec![
                entry(2, "about", "About"),
                entry(
                    2,
                    "trademarks-and-legal-notice",
                    crate::markdown::LEGAL_TITLE,
                ),
                entry(3, "legal-notice", "Legal notice"),
            ],
        };
        assert_eq!(
            legal_link(&body).expect("a link"),
            " <a href=\"#trademarks-and-legal-notice\">Trademarks and legal notice</a>."
        );
        let empty = Body {
            html: String::new(),
            toc: Vec::new(),
        };
        assert!(legal_link(&empty).is_err());
    }

    #[test]
    fn the_page_names_the_board_or_the_assembly() {
        let assembly = crate::assembly::AssemblyInfo {
            file: "a.stl".into(),
            md5: "18c4d7663073e4a8".into(),
            measured_on: String::new(),
            measured_at: String::new(),
            length_mm: 0.0,
            length_in: 0.0,
            diameter_mm: 0.0,
            diameter_in: 0.0,
            bore_in: None,
            parts: 1,
        };
        let product = Product::Assembly {
            info: &assembly,
            name: "Feather & Co",
        };
        let (about, foot) = product_words(&product, &BTreeMap::new());
        assert_eq!(about, "the Feather &amp; Co assembly at 18c4d766");
        assert_eq!(
            foot,
            "The Feather &amp; Co assembly at <code>18c4d766</code>"
        );
        let board = crate::board::BoardInfo {
            file: String::new(),
            md5: "9ef1c31a00000000".into(),
            kicad: String::new(),
            measured_on: String::new(),
            measured_at: String::new(),
            width_mm: 0.0,
            height_mm: 0.0,
            counts: Vec::new(),
        };
        let product = Product::Board {
            info: &board,
            name: "Rev B",
        };
        let values = BTreeMap::from([
            ("firmware.version".to_string(), "26.41.1".to_string()),
            ("app.version".to_string(), "26.42.1".to_string()),
        ]);
        let (about, foot) = product_words(&product, &values);
        assert_eq!(
            about,
            "the Rev B board at 9ef1c31a and the firmware 26.41.1"
        );
        assert_eq!(
            foot,
            "The Rev B board at <code>9ef1c31a</code>, firmware <code>26.41.1</code>, app <code>26.42.1</code>"
        );
    }
}

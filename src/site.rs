//! The site: one manual for each board and each language, and the index page where the
//! reader picks a language for a board. The site command of main.rs writes the pages at
//! `<out>/<board>/<lang>/index.html`, and the index at `<out>/index.html`.

use crate::manifest::Social;
use crate::render::{preview_meta, theme_color_meta, Preview};
use crate::theme::Theme;
use crate::util::esc;

const CSS: &str = include_str!("../assets/index.css");
const TITLE: &str = "SnapshotPB manuals";
const DESCRIPTION: &str = "The manual of each SnapshotPB board, in each language.";

/// One board on the index page.
pub struct Board {
    /// The id of the manifest. It names the directory of the board.
    pub id: String,
    /// The title of the manifest, as the heading of the card.
    pub title: String,
    /// The logo and the emblem, as inline SVG with the class `hero-logo`.
    pub logos: String,
    /// The language of the manifest. The link of the board alone opens this language.
    pub default_lang: String,
    /// Each language that the documents hold, in sorted order.
    pub languages: Vec<String>,
}

/// The index page. Each board is a card with its logos, its title, and one link for each
/// language. The theme is the colour scheme of the first board. The link preview takes the
/// address of the site and the picture from `social`, and has no tags without it.
pub fn index(boards: &[Board], theme: &Theme, social: Option<&Social>) -> String {
    let mut cards = String::new();
    for b in boards {
        let mut links = String::new();
        for lang in &b.languages {
            let current = if *lang == b.default_lang {
                " class=\"lang default\""
            } else {
                " class=\"lang\""
            };
            links.push_str(&format!(
                "      <li><a{current} href=\"{}/{}/\" hreflang=\"{}\" lang=\"{}\">{}</a></li>\n",
                esc(&b.id),
                esc(lang),
                esc(lang),
                esc(lang),
                esc(&language_name(lang))
            ));
        }
        cards.push_str(&format!(
            "  <section class=\"board\">\n    <a class=\"logos\" href=\"{id}/{lang}/\">{logos}</a>\n    <h2><a href=\"{id}/{lang}/\">{title}</a></h2>\n    <ul class=\"langs\">\n{links}    </ul>\n  </section>\n",
            id = esc(&b.id),
            lang = esc(&b.default_lang),
            logos = b.logos,
            title = esc(&b.title),
        ));
    }
    let theme_css = theme.css();
    let preview = social
        .map(|s| {
            preview_meta(&Preview {
                title: TITLE.to_string(),
                description: DESCRIPTION.to_string(),
                url: s.site_url.clone(),
                image: s.image.clone(),
            })
        })
        .unwrap_or_default();
    let theme_color = theme_color_meta(theme);
    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{TITLE}</title>
<meta name="description" content="{DESCRIPTION}">
{preview}{theme_color}<style>
{theme_css}
{CSS}
</style>
</head>
<body>
<main>
  <h1>{TITLE}</h1>
  <p class="lead">Select a board and a language.</p>
{cards}</main>
</body>
</html>
"##
    )
}

/// A page that sends the browser to another page at once. It stands at `<board>/index.html`
/// and opens the default language of the board. A crawler does not follow the redirect, so
/// the page holds the link preview of the manual when there is one. The preview holds the
/// canonical address, so the page holds a relative one only without a preview.
pub fn redirect(to: &str, preview: Option<&Preview>) -> String {
    let to = esc(to);
    let head = match preview {
        Some(p) => preview_meta(p),
        None => format!("<link rel=\"canonical\" href=\"{to}\">\n"),
    };
    format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta http-equiv=\"refresh\" content=\"0; url={to}\">\n{head}<title>SnapshotPB manual</title>\n</head>\n<body>\n<p><a href=\"{to}\">Open the manual</a></p>\n</body>\n</html>\n"
    )
}

/// The name of a language in that language, with its region when the tag has one. A tag
/// that the table does not know is shown as it is.
fn language_name(tag: &str) -> String {
    let mut parts = tag.split('-');
    let base = parts.next().unwrap_or(tag);
    let name = match base {
        "en" => "English",
        "de" => "Deutsch",
        "es" => "Español",
        "fr" => "Français",
        "it" => "Italiano",
        "pt" => "Português",
        "nl" => "Nederlands",
        "pl" => "Polski",
        "sv" => "Svenska",
        "da" => "Dansk",
        "nb" | "no" => "Norsk",
        "fi" => "Suomi",
        "cs" => "Čeština",
        "tr" => "Türkçe",
        "ru" => "Русский",
        "ja" => "日本語",
        "ko" => "한국어",
        "zh" => "中文",
        _ => return tag.to_string(),
    };
    let region = parts.find(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_uppercase()));
    match region {
        Some(r) => format!("{name} ({})", region_name(r)),
        None => name.to_string(),
    }
}

fn region_name(code: &str) -> String {
    match code {
        "US" => "United States",
        "GB" => "United Kingdom",
        "CA" => "Canada",
        "AU" => "Australia",
        "MX" => "México",
        "ES" => "España",
        "AR" => "Argentina",
        "FR" => "France",
        "BE" => "Belgique",
        "CH" => "Schweiz",
        "DE" => "Deutschland",
        "AT" => "Österreich",
        "BR" => "Brasil",
        "PT" => "Portugal",
        "CN" => "中国",
        "TW" => "台灣",
        _ => return code.to_string(),
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme() -> Theme {
        Theme {
            light: vec![("primary".into(), "#123456".into())],
            dark: vec![("primary".into(), "#654321".into())],
        }
    }

    #[test]
    fn the_index_links_each_language_of_each_board() {
        let boards = vec![
            Board {
                id: "autococker-rev-b".into(),
                title: "SnapshotPB Autococker".into(),
                logos: "<svg class=\"hero-logo\"></svg>".into(),
                default_lang: "en".into(),
                languages: vec!["de".into(), "en".into(), "es-MX".into()],
            },
            Board {
                id: "other".into(),
                title: "Other & Co".into(),
                logos: String::new(),
                default_lang: "fr".into(),
                languages: vec!["fr".into()],
            },
        ];
        let html = index(&boards, &theme(), None);
        assert!(!html.contains("og:title"));
        assert!(html.contains("href=\"autococker-rev-b/de/\""));
        assert!(html.contains("href=\"autococker-rev-b/es-MX/\""));
        assert!(html.contains("lang=\"es-MX\">Español (México)</a>"));
        assert!(html.contains("<h2><a href=\"other/fr/\">Other &amp; Co</a></h2>"));
        assert!(html.contains("class=\"lang default\" href=\"autococker-rev-b/en/\""));
        assert_eq!(html.matches("<section class=\"board\">").count(), 2);
    }

    #[test]
    fn the_redirect_names_the_target_once_in_each_place() {
        let html = redirect("en/", None);
        assert!(html.contains("content=\"0; url=en/\""));
        assert!(html.contains("<link rel=\"canonical\" href=\"en/\">"));
    }

    #[test]
    fn the_redirect_with_a_preview_has_one_canonical_address() {
        let preview = Preview {
            title: "B".into(),
            description: "A manual.".into(),
            url: "https://example.com/b/en/".into(),
            image: "https://example.com/s.png".into(),
        };
        let html = redirect("en/", Some(&preview));
        assert_eq!(html.matches("rel=\"canonical\"").count(), 1);
        assert!(html.contains("<link rel=\"canonical\" href=\"https://example.com/b/en/\">"));
        assert!(html.contains("<meta property=\"og:title\" content=\"B\">"));
    }

    #[test]
    fn the_index_preview_names_the_site() {
        let social = Social {
            site_url: "https://example.com/manuals/".into(),
            description: "A manual.".into(),
            image: "https://example.com/s.png".into(),
        };
        let html = index(&[], &theme(), Some(&social));
        assert!(
            html.contains("<meta property=\"og:url\" content=\"https://example.com/manuals/\">")
        );
        assert!(html.contains("<meta property=\"og:title\" content=\"SnapshotPB manuals\">"));
        assert!(html.contains("<meta property=\"og:image\" content=\"https://example.com/s.png\">"));
    }

    #[test]
    fn language_names() {
        assert_eq!(language_name("en"), "English");
        assert_eq!(language_name("es-MX"), "Español (México)");
        assert_eq!(language_name("fr-CA"), "Français (Canada)");
        assert_eq!(language_name("xx"), "xx");
        assert_eq!(language_name("pt-XX"), "Português (XX)");
    }
}

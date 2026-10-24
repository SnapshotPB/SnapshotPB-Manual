//! The site: one manual for each manifest and each language, and the index page where the
//! reader picks a manual and a language. The site command of main.rs writes the pages at
//! `<out>/<id>/<lang>/index.html`, and the index at `<out>/index.html`.

use anyhow::{bail, Result};

use crate::manifest::Social;
use crate::render::{preview_meta, theme_color_meta, Preview};
use crate::theme::Theme;
use crate::util::esc;

const CSS: &str = include_str!("../assets/index.css");
pub const TITLE: &str = "SnapshotPB manuals";
const DESCRIPTION: &str = "The manual of each SnapshotPB product, in each language.";

/// The heading of the section of the manuals that name no division.
const OTHER: &str = "Other manuals";

/// The class of the logo at the head of the index page.
pub const LOGO_CLASS: &str = "site-logo";
/// The class of the emblem at the head of a division section.
pub const EMBLEM_CLASS: &str = "division-emblem";

/// One manual on the index page. A manual is for a board or for an assembly.
pub struct Board {
    /// The id of the manifest. It names the directory of the board.
    pub id: String,
    /// The title of the manifest, as the heading of the card.
    pub title: String,
    /// The language of the manifest. The link of the board alone opens this language.
    pub default_lang: String,
    /// Each language that the documents hold, in sorted order.
    pub languages: Vec<String>,
}

/// One section of the index page: the manuals of one division, in the order of the
/// manifests.
pub struct Section {
    /// The name of the division, or none for the manuals that name no division.
    pub division: Option<String>,
    /// The emblem of the division, as inline SVG with the class [`EMBLEM_CLASS`].
    pub emblem: Option<String>,
    pub boards: Vec<Board>,
}

/// Puts `board` in the section of its division. The first manual of a division makes the
/// section. Each manual of one division must give the same emblem.
pub fn place(
    sections: &mut Vec<Section>,
    division: Option<String>,
    emblem: Option<String>,
    board: Board,
) -> Result<()> {
    match sections.iter_mut().find(|s| s.division == division) {
        Some(section) if section.emblem != emblem => match &division {
            Some(name) => bail!(
                "the manual `{}` has a different emblem from the manual before it in the division `{name}`",
                board.id
            ),
            None => bail!(
                "the manual `{}` has an emblem and no division, so the index cannot show it",
                board.id
            ),
        },
        Some(section) => section.boards.push(board),
        None => sections.push(Section {
            division,
            emblem,
            boards: vec![board],
        }),
    }
    Ok(())
}

/// The index page. The logo of the site is at the head of the page. Each division is a
/// section with its emblem as the heading, or its name when it has no emblem. The manuals
/// that name no division are a section with the heading [`OTHER`]. Each manual is a card with
/// its title and one link for each language. The theme is the colour scheme of the first
/// board. The link preview takes the address of the site and the picture from `social`, and
/// has no tags without it.
pub fn index(logo: &str, sections: &[Section], theme: &Theme, social: Option<&Social>) -> String {
    let mut body = String::new();
    for section in sections {
        body.push_str("  <section class=\"division\">\n");
        match (&section.emblem, &section.division) {
            (Some(emblem), _) => {
                body.push_str(&format!("    <h2 class=\"division-head\">{emblem}</h2>\n"));
            }
            (None, Some(name)) => {
                body.push_str(&format!(
                    "    <h2 class=\"division-head\">{}</h2>\n",
                    esc(name)
                ));
            }
            (None, None) => {
                body.push_str(&format!("    <h2 class=\"division-head\">{OTHER}</h2>\n"));
            }
        }
        body.push_str("    <div class=\"boards\">\n");
        for b in &section.boards {
            body.push_str(&card(b));
        }
        body.push_str("    </div>\n  </section>\n");
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
  <header class="site-head">
    <h1>{logo}</h1>
    <p class="lead">Select a manual and a language.</p>
  </header>
{body}</main>
</body>
</html>
"##
    )
}

/// The card of one manual: its title, and one link for each language.
fn card(b: &Board) -> String {
    let mut links = String::new();
    for lang in &b.languages {
        let current = if *lang == b.default_lang {
            " class=\"lang default\""
        } else {
            " class=\"lang\""
        };
        links.push_str(&format!(
            "          <li><a{current} href=\"{}/{}/\" hreflang=\"{}\" lang=\"{}\">{}</a></li>\n",
            esc(&b.id),
            esc(lang),
            esc(lang),
            esc(lang),
            esc(&language_name(lang))
        ));
    }
    format!(
        "      <article class=\"board\">\n        <h3><a href=\"{id}/{lang}/\">{title}</a></h3>\n        <ul class=\"langs\">\n{links}        </ul>\n      </article>\n",
        id = esc(&b.id),
        lang = esc(&b.default_lang),
        title = esc(&b.title),
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

    fn board(id: &str, title: &str, default_lang: &str, languages: &[&str]) -> Board {
        Board {
            id: id.into(),
            title: title.into(),
            default_lang: default_lang.into(),
            languages: languages.iter().map(|l| l.to_string()).collect(),
        }
    }

    const EMBLEM: &str = "<svg class=\"division-emblem\" aria-label=\"Autococker Division\"></svg>";

    #[test]
    fn the_index_links_each_language_of_each_board() {
        let mut sections = Vec::new();
        for (division, emblem, b) in [
            (
                Some("Autococker Division"),
                Some(EMBLEM),
                board(
                    "autococker-rev-b",
                    "SnapshotPB Autococker",
                    "en",
                    &["de", "en", "es-MX"],
                ),
            ),
            (None, None, board("other", "Other & Co", "fr", &["fr"])),
        ] {
            place(
                &mut sections,
                division.map(String::from),
                emblem.map(String::from),
                b,
            )
            .expect("a section");
        }
        let html = index("<svg class=\"site-logo\"></svg>", &sections, &theme(), None);
        assert!(!html.contains("og:title"));
        assert!(html.contains("href=\"autococker-rev-b/de/\""));
        assert!(html.contains("href=\"autococker-rev-b/es-MX/\""));
        assert!(html.contains("lang=\"es-MX\">Español (México)</a>"));
        assert!(html.contains("<h3><a href=\"other/fr/\">Other &amp; Co</a></h3>"));
        assert!(html.contains("class=\"lang default\" href=\"autococker-rev-b/en/\""));
        assert_eq!(html.matches("<article class=\"board\">").count(), 2);
        assert_eq!(html.matches("<section class=\"division\">").count(), 2);
    }

    #[test]
    fn the_logo_is_at_the_head_of_the_page_once_and_the_emblem_heads_its_division() {
        let mut sections = Vec::new();
        for id in ["autococker-rev-b", "featherweight-ics"] {
            place(
                &mut sections,
                Some("Autococker Division".into()),
                Some(EMBLEM.into()),
                board(id, id, "en", &["en"]),
            )
            .expect("a section");
        }
        let html = index("<svg class=\"site-logo\"></svg>", &sections, &theme(), None);
        assert_eq!(html.matches("class=\"site-logo\"").count(), 1);
        assert_eq!(html.matches("class=\"division-emblem\"").count(), 1);
        assert_eq!(html.matches("<section class=\"division\">").count(), 1);
        assert_eq!(html.matches("<article class=\"board\">").count(), 2);
        let logo = html.find("class=\"site-logo\"").expect("the logo");
        let emblem = html.find("class=\"division-emblem\"").expect("the emblem");
        let first = html.find("autococker-rev-b/en/").expect("the first manual");
        assert!(logo < emblem && emblem < first);
        assert!(html.contains(&format!("<h2 class=\"division-head\">{EMBLEM}</h2>")));
    }

    #[test]
    fn each_division_is_its_own_section_in_the_order_of_the_manifests() {
        let mut sections = Vec::new();
        for (division, id) in [
            (Some("Autococker Division"), "a"),
            (Some("Other Division"), "b"),
            (None, "c"),
            (Some("Autococker Division"), "d"),
        ] {
            place(
                &mut sections,
                division.map(String::from),
                None,
                board(id, id, "en", &["en"]),
            )
            .expect("a section");
        }
        let ids: Vec<(Option<&str>, Vec<&str>)> = sections
            .iter()
            .map(|s| {
                (
                    s.division.as_deref(),
                    s.boards.iter().map(|b| b.id.as_str()).collect(),
                )
            })
            .collect();
        assert_eq!(
            ids,
            vec![
                (Some("Autococker Division"), vec!["a", "d"]),
                (Some("Other Division"), vec!["b"]),
                (None, vec!["c"]),
            ]
        );
        let html = index("", &sections, &theme(), None);
        assert!(html.contains("<h2 class=\"division-head\">Other Division</h2>"));
        assert!(html.contains("<h2 class=\"division-head\">Other manuals</h2>"));
        assert_eq!(html.matches("<h2 ").count(), 3);
    }

    #[test]
    fn a_division_refuses_a_second_emblem() {
        let mut sections = Vec::new();
        place(
            &mut sections,
            Some("Autococker Division".into()),
            Some(EMBLEM.into()),
            board("a", "a", "en", &["en"]),
        )
        .expect("the first manual");
        let refused = place(
            &mut sections,
            Some("Autococker Division".into()),
            Some("<svg></svg>".into()),
            board("b", "b", "en", &["en"]),
        )
        .expect_err("a different emblem");
        assert!(refused.to_string().contains("`b`"), "{refused}");
        assert_eq!(sections[0].boards.len(), 1);
    }

    #[test]
    fn a_division_refuses_an_emblem_after_a_manual_with_none() {
        let mut sections = Vec::new();
        place(
            &mut sections,
            Some("Autococker Division".into()),
            None,
            board("a", "a", "en", &["en"]),
        )
        .expect("the first manual");
        place(
            &mut sections,
            Some("Autococker Division".into()),
            Some(EMBLEM.into()),
            board("b", "b", "en", &["en"]),
        )
        .expect_err("an emblem that the first manual does not give");
        let refused = place(
            &mut sections,
            None,
            Some(EMBLEM.into()),
            board("c", "c", "en", &["en"]),
        );
        assert!(
            refused.is_ok(),
            "the first manual with no division makes its section"
        );
        let refused = place(&mut sections, None, None, board("d", "d", "en", &["en"]))
            .expect_err("a second emblem in the section with no division");
        assert!(refused.to_string().contains("no division"), "{refused}");
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
        let html = index("", &[], &theme(), Some(&social));
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

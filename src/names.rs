//! The readable names of the things that a board labels by a designator. The `[names]`
//! table of the manifest maps a designator such as `J11` to a name such as `Trigger board`.
//! The page shows the name, with the designator after it in parentheses, so that a reader
//! can find the label on the board. A label in a drawing has little room, so it shows the
//! name alone. The names apply to each code span of the page, from a chapter or from a
//! document, and to each label of an inline SVG.

use std::collections::BTreeMap;

use anyhow::{bail, Context, Result};
use regex::{Captures, Regex};

use crate::util::esc;

/// The readable names of the designators of one manifest, with the patterns that apply them.
pub struct Names {
    map: BTreeMap<String, String>,
    /// A code span of the page, and the `<pre>` before it when the span is a code block.
    re_code: Regex,
    /// The text of a label in an inline SVG.
    re_label: Regex,
    /// Each designator as a whole word, for the text of a label.
    re_word: Option<Regex>,
}

impl Names {
    /// The names of a manifest, ready to apply.
    ///
    /// # Errors
    ///
    /// A designator must start and end with a letter, a digit or an underscore, and it
    /// can hold a dash or a dot between them, such as `USB-C`. Another designator is an
    /// error, because the page could not find it as a whole word.
    pub fn new(map: &BTreeMap<String, String>) -> Result<Names> {
        for key in map.keys() {
            if !is_designator(key) {
                bail!("the name key `{key}` is not a designator: it must start and end with a letter, a digit or an underscore, and hold only letters, digits, `_`, `-` and `.`");
            }
        }
        let re_code = Regex::new(r"(<pre>)?<code>([^<]+)</code>").context("the code pattern")?;
        let re_label =
            Regex::new(r"(<(?:text|tspan)\b[^>]*>)([^<]*)").context("the label pattern")?;
        let re_word = if map.is_empty() {
            None
        } else {
            let mut keys: Vec<&str> = map.keys().map(String::as_str).collect();
            keys.sort_by_key(|k| std::cmp::Reverse(k.len()));
            let alternation: Vec<String> = keys.iter().map(|k| regex::escape(k)).collect();
            Some(
                Regex::new(&format!(r"\b(?:{})\b", alternation.join("|")))
                    .context("the designator pattern")?,
            )
        };
        Ok(Names {
            map: map.clone(),
            re_code,
            re_label,
            re_word,
        })
    }

    /// The page with each named designator replaced: a code span that holds one designator
    /// becomes the name with the designator after it, and a label of an inline SVG shows
    /// the name in place of each designator in it. A code block and a code span with a
    /// class stay as they are.
    pub fn apply(&self, html: &str) -> String {
        let re_word = match &self.re_word {
            Some(re) => re,
            None => return html.to_string(),
        };
        let html = self.re_code.replace_all(html, |cap: &Captures| {
            if cap.get(1).is_some() {
                return cap[0].to_string();
            }
            match self.map.get(&cap[2]) {
                Some(name) => format!(
                    "<span class=\"named\">{} (<code class=\"ref\">{}</code>)</span>",
                    esc(name),
                    &cap[2]
                ),
                None => cap[0].to_string(),
            }
        });
        self.re_label
            .replace_all(&html, |cap: &Captures| {
                let text = re_word.replace_all(&cap[2], |w: &Captures| match self.map.get(&w[0]) {
                    Some(name) => esc(name),
                    None => w[0].to_string(),
                });
                format!("{}{text}", &cap[1])
            })
            .into_owned()
    }
}

/// True when a key can stand as a whole word in a page: it starts and ends with a word
/// character, and it holds no white space and no character that HTML escapes.
fn is_designator(key: &str) -> bool {
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let first = key.chars().next().is_some_and(word);
    let last = key.chars().last().is_some_and(word);
    first && last && key.chars().all(|c| word(c) || c == '-' || c == '.')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names() -> Names {
        Names::new(&BTreeMap::from([
            ("J11".to_string(), "Trigger board".to_string()),
            ("TRIG".to_string(), "Trigger".to_string()),
            ("TRIG1".to_string(), "Trigger".to_string()),
            ("USB-C".to_string(), "USB port".to_string()),
        ]))
        .unwrap()
    }

    #[test]
    fn a_code_span_with_a_designator_shows_the_name() {
        let html = names().apply("<p>Connect <code>J11</code> to <code>BATT</code>.</p>");
        assert_eq!(
            html,
            "<p>Connect <span class=\"named\">Trigger board (<code class=\"ref\">J11</code>)</span> to <code>BATT</code>.</p>"
        );
    }

    #[test]
    fn a_code_block_and_a_code_span_with_a_class_stay() {
        let n = names();
        assert_eq!(
            n.apply("<pre><code>J11</code></pre>"),
            "<pre><code>J11</code></pre>"
        );
        assert_eq!(
            n.apply("<code class=\"key\">J11</code>"),
            "<code class=\"key\">J11</code>"
        );
    }

    #[test]
    fn a_label_of_a_drawing_shows_the_name_by_whole_word() {
        let html = names().apply(
            "<svg><text x='1'>socket J11</text><text>header TRIG1</text><tspan>TRIG</tspan><text>TRIGGER</text></svg>",
        );
        assert_eq!(
            html,
            "<svg><text x='1'>socket Trigger board</text><text>header Trigger</text><tspan>Trigger</tspan><text>TRIGGER</text></svg>"
        );
    }

    #[test]
    fn the_name_is_escaped_and_a_designator_can_hold_a_dash() {
        let n = Names::new(&BTreeMap::from([(
            "J1".to_string(),
            "A <b> name".to_string(),
        )]))
        .unwrap();
        assert_eq!(n.apply("<text>J1</text>"), "<text>A &lt;b&gt; name</text>");
        assert_eq!(
            names().apply("<text>the USB-C port</text>"),
            "<text>the USB port port</text>"
        );
    }

    #[test]
    fn a_key_that_is_not_a_designator_is_rejected() {
        for bad in ["", " ", "J 11", "+5V", "Q&A", "<b>", "TRIG-", "-TRIG", "ÄB"] {
            let map = BTreeMap::from([(bad.to_string(), "Name".to_string())]);
            assert!(Names::new(&map).is_err(), "{bad:?} must be rejected");
        }
        for good in ["J11", "USB-C", "net.3v3", "_x"] {
            let map = BTreeMap::from([(good.to_string(), "Name".to_string())]);
            assert!(Names::new(&map).is_ok(), "{good:?} must be accepted");
        }
    }

    #[test]
    fn no_names_leave_the_page_as_it_is() {
        let n = Names::new(&BTreeMap::new()).unwrap();
        assert_eq!(n.apply("<code>J11</code>"), "<code>J11</code>");
    }
}

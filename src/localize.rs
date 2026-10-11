//! The choice of one language from the localized text of a document.
//!
//! A document carries each text that the manual prints as an array of `{lang, text}` items.
//! The manual is built for one language. Before the typed model reads a document, this
//! module replaces each such array with one string: the text of the language of the manual,
//! or the nearest fallback. The model then holds the words of one language, and the rest of
//! the generator does not know about languages.

use std::collections::BTreeSet;

use anyhow::{bail, Result};
use serde_json::{Map, Value};

/// The language that each localized text must have, and the last fallback.
pub const FALLBACK: &str = "en";

/// The value with each localized array replaced by the text of `lang`.
///
/// The choice for one array, in this order: the item with the language `lang`; the first
/// item with the same language and another region or no region, so `es` stands in for
/// `es-MX`; the English item. The `params` member of a media item is free-form, and it
/// stays as it is.
///
/// # Errors
///
/// An array that names one language twice, or that has no English item.
pub fn localize(value: Value, lang: &str) -> Result<Value> {
    walk(value, lang, "")
}

fn walk(value: Value, lang: &str, path: &str) -> Result<Value> {
    match value {
        Value::Object(map) => {
            let mut out = Map::with_capacity(map.len());
            for (k, v) in map {
                let v = if k == "params" && is_media_item(path) {
                    v
                } else {
                    walk(v, lang, &format!("{path}/{k}"))?
                };
                out.insert(k, v);
            }
            Ok(Value::Object(out))
        }
        Value::Array(items) => {
            if let Some(texts) = as_localized(&items) {
                return pick(&texts, lang, path).map(|t| Value::String(t.to_string()));
            }
            let mut out = Vec::with_capacity(items.len());
            for (i, v) in items.into_iter().enumerate() {
                out.push(walk(v, lang, &format!("{path}/{i}"))?);
            }
            Ok(Value::Array(out))
        }
        other => Ok(other),
    }
}

/// Each language tag that the localized texts of a document name, in sorted order. The
/// site builds one manual for each of them.
pub fn languages(value: &Value) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    collect(value, "", &mut out);
    out
}

/// The same walk as `walk`, with the same `params` exclusion. A change to one goes to the
/// other.
fn collect(value: &Value, path: &str, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                if k == "params" && is_media_item(path) {
                    continue;
                }
                collect(v, &format!("{path}/{k}"), out);
            }
        }
        Value::Array(items) => {
            if let Some(texts) = as_localized(items) {
                out.extend(texts.iter().map(|(l, _)| l.to_string()));
                return;
            }
            for (i, v) in items.iter().enumerate() {
                collect(v, &format!("{path}/{i}"), out);
            }
        }
        _ => {}
    }
}

/// True for the path of one media item, `/media/<key>`, whose `params` member is free-form.
fn is_media_item(path: &str) -> bool {
    path.strip_prefix("/media/")
        .is_some_and(|key| !key.is_empty() && !key.contains('/'))
}

/// True for a language tag as the schema set defines it: a language of two or three lower
/// case letters, and then zero or more subtags of two to eight letters or digits.
pub fn is_language_tag(tag: &str) -> bool {
    let mut parts = tag.split('-');
    let Some(language) = parts.next() else {
        return false;
    };
    let language_ok =
        (2..=3).contains(&language.len()) && language.chars().all(|c| c.is_ascii_lowercase());
    language_ok
        && parts.all(|p| (2..=8).contains(&p.len()) && p.chars().all(|c| c.is_ascii_alphanumeric()))
}

/// The `(lang, text)` pairs of an array when each item is a `{lang, text}` object with
/// nothing else in it. No other array of the schema set has that shape, and a later schema
/// must not add one: see the section Localized text of docs/firmware-schema.md.
fn as_localized(items: &[Value]) -> Option<Vec<(&str, &str)>> {
    if items.is_empty() {
        return None;
    }
    items
        .iter()
        .map(|item| {
            let o = item.as_object()?;
            if o.len() != 2 {
                return None;
            }
            Some((o.get("lang")?.as_str()?, o.get("text")?.as_str()?))
        })
        .collect()
}

/// The text for `lang`, with the fallbacks that the module doc describes.
fn pick<'a>(texts: &[(&'a str, &'a str)], lang: &str, path: &str) -> Result<&'a str> {
    for (i, (l, _)) in texts.iter().enumerate() {
        if texts[..i].iter().any(|(m, _)| m == l) {
            bail!("the text at {path} has the language `{l}` twice");
        }
    }
    let Some(english) = texts.iter().find(|(l, _)| *l == FALLBACK) else {
        bail!("the text at {path} has no English item, and the manual needs one as the fallback");
    };
    let base = base_of(lang);
    let found = texts
        .iter()
        .find(|(l, _)| *l == lang)
        .or_else(|| texts.iter().find(|(l, _)| base_of(l) == base))
        .unwrap_or(english);
    Ok(found.1)
}

/// The language without its region: `es` for `es-MX`.
fn base_of(lang: &str) -> &str {
    lang.split('-').next().unwrap_or(lang)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn doc() -> Value {
        json!({
            "title": [{"lang": "en", "text": "Fire mode"}, {"lang": "de", "text": "Feuermodus"}, {"lang": "es-MX", "text": "Modo de disparo"}],
            "items": [{"render": [{"lang": "en", "text": "Mode"}], "key": "a.b"}],
            "media": {"m": {"caption": [{"lang": "en", "text": "A picture"}], "params": [{"lang": "x", "text": "stays"}]}},
            "values": ["a", "b"],
            "n": 3
        })
    }

    #[test]
    fn the_language_of_the_manual_wins() {
        let v = localize(doc(), "de").unwrap();
        assert_eq!(v["title"], "Feuermodus");
        assert_eq!(v["items"][0]["render"], "Mode");
        assert_eq!(v["media"]["m"]["caption"], "A picture");
    }

    #[test]
    fn a_region_falls_back_to_its_language_and_then_to_english() {
        let v = localize(doc(), "es").unwrap();
        assert_eq!(v["title"], "Modo de disparo");
        let v = localize(doc(), "de-AT").unwrap();
        assert_eq!(v["title"], "Feuermodus");
        let v = localize(doc(), "fr").unwrap();
        assert_eq!(v["title"], "Fire mode");
    }

    #[test]
    fn other_arrays_and_params_stay() {
        let v = localize(doc(), "en").unwrap();
        assert_eq!(v["values"], json!(["a", "b"]));
        assert_eq!(v["n"], 3);
        assert_eq!(
            v["media"]["m"]["params"],
            json!([{"lang": "x", "text": "stays"}])
        );
    }

    #[test]
    fn only_the_params_of_a_media_item_stay() {
        let v = json!({
            "screens": {"params": {"title": [{"lang": "en", "text": "Params"}]}},
            "media": {"m": {"params": {"deep": [{"lang": "en", "text": "stays"}]}}}
        });
        let v = localize(v, "en").unwrap();
        assert_eq!(v["screens"]["params"]["title"], "Params");
        assert_eq!(
            v["media"]["m"]["params"]["deep"],
            json!([{"lang": "en", "text": "stays"}])
        );
    }

    #[test]
    fn a_language_tag_has_the_shape_of_the_schema() {
        for ok in ["en", "de", "es-MX", "fr-CA", "ast", "zh-Hant-TW"] {
            assert!(is_language_tag(ok), "{ok}");
        }
        for bad in ["", "DE", "e", "en-", "en-x", "en_US", "deutsch", "es-MX-"] {
            assert!(!is_language_tag(bad), "{bad}");
        }
    }

    #[test]
    fn a_repeated_language_and_a_missing_english_are_errors() {
        let twice = json!({"t": [{"lang": "en", "text": "a"}, {"lang": "en", "text": "b"}]});
        assert!(localize(twice, "en")
            .unwrap_err()
            .to_string()
            .contains("twice"));
        let no_en = json!({"s": {"t": [{"lang": "de", "text": "a"}]}});
        let err = localize(no_en, "de").unwrap_err().to_string();
        assert!(err.contains("/s/t") && err.contains("English"), "{err}");
    }

    #[test]
    fn the_languages_are_each_tag_of_each_localized_text() {
        let langs = languages(&doc());
        assert_eq!(
            langs.into_iter().collect::<Vec<_>>(),
            vec!["de", "en", "es-MX"]
        );
    }
}

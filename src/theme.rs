//! The colour scheme of the app, read from its Kotlin source.

use std::path::Path;

use anyhow::{bail, Result};
use regex::Regex;

use crate::util::{kebab, read_text};

pub struct Theme {
    /// `(css-name, #rrggbb)` pairs of the light scheme, in file order.
    pub light: Vec<(String, String)>,
    pub dark: Vec<(String, String)>,
}

impl Theme {
    pub fn parse(path: &Path) -> Result<Theme> {
        let text = read_text(path)?;
        let re = Regex::new(
            r"val\s+(\w+?)(Light|Dark)\s*=\s*Color(?:\((0x[0-9A-Fa-f]{8})\)|\.(White|Black))",
        )
        .unwrap();
        let mut light = Vec::new();
        let mut dark = Vec::new();
        for cap in re.captures_iter(&text) {
            let name = kebab(&cap[1]);
            let value = match (cap.get(3), cap.get(4)) {
                (Some(hex), _) => format!("#{}", &hex.as_str()[4..]),
                (_, Some(named)) if named.as_str() == "White" => "#ffffff".to_string(),
                (_, Some(_)) => "#000000".to_string(),
                _ => unreachable!(),
            };
            let value = value.to_lowercase();
            match &cap[2] {
                "Light" => light.push((name, value)),
                _ => dark.push((name, value)),
            }
        }
        if light.is_empty() || dark.is_empty() {
            bail!(
                "{} holds no `val <name>Light = Color(0x…)` and `val <name>Dark = Color(0x…)` pairs",
                path.display()
            );
        }
        for want in [
            "primary",
            "on-primary",
            "background",
            "on-background",
            "surface",
            "outline",
        ] {
            if !light.iter().any(|(n, _)| n == want) {
                bail!(
                    "the light scheme of {} has no `{want}` colour",
                    path.display()
                );
            }
        }
        Ok(Theme { light, dark })
    }

    /// The CSS custom properties: light on `:root`, dark on `[data-theme="dark"]` and on a
    /// system that prefers dark, unless the page says `data-theme="light"`.
    pub fn css(&self) -> String {
        let block = |pairs: &[(String, String)]| {
            pairs
                .iter()
                .map(|(n, v)| format!("  --md-{n}: {v};\n"))
                .collect::<String>()
        };
        let mut css = String::new();
        css.push_str(":root {\n  color-scheme: light;\n");
        css.push_str(&block(&self.light));
        css.push_str("}\n");
        css.push_str(":root[data-theme=\"dark\"] {\n  color-scheme: dark;\n");
        css.push_str(&block(&self.dark));
        css.push_str("}\n");
        css.push_str("@media (prefers-color-scheme: dark) {\n  :root:not([data-theme=\"light\"]) {\n    color-scheme: dark;\n");
        for (n, v) in &self.dark {
            css.push_str(&format!("    --md-{n}: {v};\n"));
        }
        css.push_str("  }\n}\n");
        css
    }
}

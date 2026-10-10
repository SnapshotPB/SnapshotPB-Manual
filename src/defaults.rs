//! Numbers that the firmware fixes at build time: Kconfig defaults and C defines.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Result};
use regex::Regex;

use crate::util::read_text;

#[derive(Default)]
pub struct Defaults {
    pub kconfig: BTreeMap<String, String>,
    pub defines: BTreeMap<String, String>,
}

impl Defaults {
    pub fn add_kconfig(&mut self, path: &Path) -> Result<()> {
        let text = read_text(path)?;
        let re_cfg = Regex::new(r"^\s*(?:menu)?config\s+(\w+)\s*$").unwrap();
        let re_def = Regex::new(r##"^\s*default\s+"?([^"#\s]+)"?"##).unwrap();
        let mut current: Option<String> = None;
        for line in text.lines() {
            if let Some(cap) = re_cfg.captures(line) {
                current = Some(cap[1].to_string());
                continue;
            }
            if let (Some(name), Some(cap)) = (&current, re_def.captures(line)) {
                self.kconfig
                    .entry(name.clone())
                    .or_insert_with(|| cap[1].to_string());
            }
        }
        Ok(())
    }

    pub fn add_defines(&mut self, path: &Path) -> Result<()> {
        let text = read_text(path)?;
        let re = Regex::new(r"^\s*#\s*define\s+(\w+)\s+(-?\d+)[uUlL]*\b").unwrap();
        for line in text.lines() {
            if let Some(cap) = re.captures(line) {
                self.defines
                    .entry(cap[1].to_string())
                    .or_insert_with(|| cap[2].to_string());
            }
        }
        Ok(())
    }

    /// A default by its source: `kconfig:NAME`, `define:NAME`, `literal:TEXT` or
    /// `enum:SYMBOL`. The result is `(raw text, is_enum_symbol)`.
    pub fn resolve(&self, spec: &str) -> Result<(String, bool)> {
        let (kind, name) = match spec.split_once(':') {
            Some(p) => p,
            None => bail!("a default needs a `kind:name` form, and `{spec}` has none"),
        };
        match kind {
            "kconfig" => match self.kconfig.get(name) {
                Some(v) => Ok((v.clone(), false)),
                None => bail!("no Kconfig default `{name}` in the files that the manifest lists"),
            },
            "define" => match self.defines.get(name) {
                Some(v) => Ok((v.clone(), false)),
                None => bail!("no numeric `#define {name}` in the files that the manifest lists"),
            },
            "literal" => Ok((name.to_string(), false)),
            "enum" => Ok((name.to_string(), true)),
            other => bail!("unknown default kind `{other}` in `{spec}`"),
        }
    }
}

//! A mechanical assembly, as the tools of its repository measured it into `METRICS.json`.

use std::path::Path;

use anyhow::{bail, Context, Result};
use serde_json::Value;

use crate::util::read_text;

pub struct AssemblyInfo {
    /// The model file that the tools measured.
    pub file: String,
    pub md5: String,
    pub measured_on: String,
    pub measured_at: String,
    pub length_mm: f64,
    pub length_in: f64,
    pub diameter_mm: f64,
    pub diameter_in: f64,
    /// The bore that the assembly fits, as the metrics file writes it, such as `11/16`.
    pub bore_in: Option<String>,
    /// The number of parts.
    pub parts: usize,
}

impl AssemblyInfo {
    pub fn load(path: &Path) -> Result<AssemblyInfo> {
        let text = read_text(path)?;
        let v: Value =
            serde_json::from_str(&text).with_context(|| format!("JSON of {}", path.display()))?;
        AssemblyInfo::from_value(&v).with_context(|| path.display().to_string())
    }

    fn from_value(v: &Value) -> Result<AssemblyInfo> {
        let meta = &v["meta"];
        let md5 = meta["assembly_md5"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        if md5.len() < 8 || !md5.is_ascii() {
            bail!("the metrics have no meta.assembly_md5");
        }
        let envelope = &v["envelope"];
        let number = |key: &str| -> Result<f64> {
            envelope[key]
                .as_f64()
                .with_context(|| format!("the metrics have no envelope.{key}"))
        };
        let parts = v["parts"]
            .as_object()
            .context("the metrics have no parts object")?
            .len();
        Ok(AssemblyInfo {
            file: meta["assembly"].as_str().unwrap_or_default().to_string(),
            md5,
            measured_on: meta["measured_on"].as_str().unwrap_or_default().to_string(),
            measured_at: meta["measured_at_commit"]
                .as_str()
                .unwrap_or_default()
                .chars()
                .take(7)
                .collect(),
            length_mm: number("length_mm")?,
            length_in: number("length_in")?,
            diameter_mm: number("diameter_mm")?,
            diameter_in: number("diameter_in")?,
            bore_in: envelope["bore_in"].as_str().map(String::from),
            parts,
        })
    }

    pub fn md5_short(&self) -> &str {
        &self.md5[..8]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_metrics_give_the_envelope_and_the_parts() {
        let v: Value = serde_json::from_str(
            r#"{
              "meta": {"assembly": "a.stl", "assembly_md5": "18c4d7663073e4a8", "measured_at_commit": "a082d47abc", "measured_on": "2026-10-23"},
              "envelope": {"length_in": 2.625, "length_mm": 66.68, "diameter_in": 0.6835, "diameter_mm": 17.36, "bore_in": "11/16"},
              "parts": {"hammer": {"label": "Hammer", "material": "Delrin"}, "rod": {"material": "Steel"}}
            }"#,
        )
        .expect("JSON");
        let a = AssemblyInfo::from_value(&v).expect("the metrics");
        assert_eq!(a.md5_short(), "18c4d766");
        assert_eq!(a.measured_at, "a082d47");
        assert_eq!(a.bore_in.as_deref(), Some("11/16"));
        assert_eq!(a.parts, 2);
        assert_eq!(a.length_mm, 66.68);
        let err = |v: Value| match AssemblyInfo::from_value(&v) {
            Ok(_) => panic!("the metrics are not complete"),
            Err(e) => e.to_string(),
        };
        assert!(err(serde_json::json!({"meta": {}})).contains("assembly_md5"));
        let mut no_parts = v.clone();
        no_parts.as_object_mut().expect("an object").remove("parts");
        assert!(err(no_parts).contains("parts"));
        let mut no_length = v.clone();
        no_length["envelope"]
            .as_object_mut()
            .expect("an object")
            .remove("length_mm");
        assert!(err(no_length).contains("envelope.length_mm"));
    }
}

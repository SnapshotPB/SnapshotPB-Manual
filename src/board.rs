//! The board, as `tools/verify_board.py` measured it into `METRICS.json`.

use std::path::Path;

use anyhow::{bail, Context, Result};
use serde_json::Value;

use crate::util::read_text;

pub struct BoardInfo {
    pub file: String,
    pub md5: String,
    pub kicad: String,
    pub measured_on: String,
    pub measured_at: String,
    pub width_mm: f64,
    pub height_mm: f64,
    /// `(name, value)` of the counts block, in key order.
    pub counts: Vec<(String, i64)>,
}

impl BoardInfo {
    pub fn load(path: &Path) -> Result<BoardInfo> {
        let text = read_text(path)?;
        let v: Value =
            serde_json::from_str(&text).with_context(|| format!("JSON of {}", path.display()))?;
        let meta = &v["meta"];
        let md5 = meta["board_md5"].as_str().unwrap_or_default().to_string();
        if md5.len() < 8 {
            bail!("{} has no meta.board_md5", path.display());
        }
        let outline = &v["outline_mm"];
        let mut counts = Vec::new();
        if let Some(m) = v["counts"].as_object() {
            for (k, n) in m {
                if let Some(i) = n.as_i64() {
                    counts.push((k.clone(), i));
                }
            }
        }
        Ok(BoardInfo {
            file: meta["board"].as_str().unwrap_or_default().to_string(),
            md5,
            kicad: meta["kicad"].as_str().unwrap_or_default().to_string(),
            measured_on: meta["measured_on"].as_str().unwrap_or_default().to_string(),
            measured_at: meta["measured_at_commit"]
                .as_str()
                .unwrap_or_default()
                .chars()
                .take(7)
                .collect(),
            width_mm: outline["width"].as_f64().unwrap_or(0.0),
            height_mm: outline["height"].as_f64().unwrap_or(0.0),
            counts,
        })
    }

    pub fn md5_short(&self) -> &str {
        &self.md5[..8]
    }
}

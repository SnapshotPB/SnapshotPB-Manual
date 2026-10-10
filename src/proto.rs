//! The protocol messages and enums, with the comment that documents each field and value.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Result};
use regex::Regex;

use crate::util::read_text;

#[derive(Debug, Clone)]
pub struct EnumValue {
    pub symbol: String,
    pub doc: String,
}

#[derive(Debug, Clone)]
pub struct EnumDef {
    pub doc: String,
    pub values: Vec<EnumValue>,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub doc: String,
}

#[derive(Debug, Clone)]
pub struct MessageDef {
    pub fields: Vec<Field>,
}

#[derive(Default)]
pub struct ProtoModel {
    pub enums: BTreeMap<String, EnumDef>,
    pub messages: BTreeMap<String, MessageDef>,
    pub files: Vec<String>,
}

impl ProtoModel {
    /// Reads every `.proto` file under a root.
    pub fn load(root: &Path) -> Result<ProtoModel> {
        let mut model = ProtoModel::default();
        let mut paths = Vec::new();
        walk(root, &mut paths)?;
        paths.sort();
        if paths.is_empty() {
            bail!("no .proto file under {}", root.display());
        }
        for p in paths {
            let text = read_text(&p)?;
            model.parse(&text);
            model
                .files
                .push(p.strip_prefix(root).unwrap_or(&p).display().to_string());
        }
        Ok(model)
    }

    fn parse(&mut self, text: &str) {
        let re_field = Regex::new(
            r"^\s*(?:repeated\s+|optional\s+)?([\w.]+)\s+(\w+)\s*=\s*(\d+)\s*(?:\[[^\]]*\])?\s*;",
        )
        .unwrap();
        let re_value = Regex::new(r"^\s*(\w+)\s*=\s*(-?\d+)\s*(?:\[[^\]]*\])?\s*;").unwrap();
        let re_open = Regex::new(r"^\s*(message|enum|oneof)\s+(\w+)\s*\{").unwrap();
        let mut doc: Vec<String> = Vec::new();
        // The open containers: ("message"|"enum"|"oneof", name).
        let mut stack: Vec<(String, String)> = Vec::new();
        for raw in text.lines() {
            let line = raw.trim_end();
            let trimmed = line.trim_start();
            if let Some(c) = trimmed.strip_prefix("//") {
                doc.push(c.strip_prefix(' ').unwrap_or(c).to_string());
                continue;
            }
            if trimmed.is_empty() {
                doc.clear();
                continue;
            }
            if trimmed.starts_with("package ")
                || trimmed.starts_with("import ")
                || trimmed.starts_with("syntax ")
                || trimmed.starts_with("option ")
            {
                doc.clear();
                continue;
            }
            if let Some(cap) = re_open.captures(trimmed) {
                let kind = cap[1].to_string();
                let name = cap[2].to_string();
                let d = join_doc(&doc);
                doc.clear();
                match kind.as_str() {
                    "message" => {
                        self.messages
                            .insert(name.clone(), MessageDef { fields: Vec::new() });
                    }
                    "enum" => {
                        self.enums.insert(
                            name.clone(),
                            EnumDef {
                                doc: d,
                                values: Vec::new(),
                            },
                        );
                    }
                    _ => {}
                }
                stack.push((kind, name));
                continue;
            }
            if trimmed.starts_with('}') {
                stack.pop();
                doc.clear();
                continue;
            }
            // The nearest message or enum that is open.
            let owner = stack.iter().rev().find(|(k, _)| k != "oneof").cloned();
            match owner {
                Some((kind, name)) if kind == "enum" => {
                    if let Some(cap) = re_value.captures(trimmed) {
                        if let Some(e) = self.enums.get_mut(&name) {
                            e.values.push(EnumValue {
                                symbol: cap[1].to_string(),
                                doc: join_doc(&doc),
                            });
                        }
                    }
                }
                Some((kind, name)) if kind == "message" => {
                    if let Some(cap) = re_field.captures(trimmed) {
                        if let Some(m) = self.messages.get_mut(&name) {
                            m.fields.push(Field {
                                name: cap[2].to_string(),
                                doc: join_doc(&doc),
                            });
                        }
                    }
                }
                _ => {}
            }
            doc.clear();
        }
    }

    /// Finds an enum by its short name or by its full name.
    pub fn enum_def(&self, name: &str) -> Option<&EnumDef> {
        let short = name.rsplit('.').next().unwrap_or(name);
        self.enums.get(short)
    }

    pub fn field(&self, message: &str, field: &str) -> Option<&Field> {
        self.messages
            .get(message)?
            .fields
            .iter()
            .find(|f| f.name == field)
    }
}

/// Joins comment lines into paragraphs: a blank comment line separates two paragraphs.
fn join_doc(lines: &[String]) -> String {
    let mut paras: Vec<String> = Vec::new();
    let mut cur = String::new();
    for l in lines {
        if l.trim().is_empty() {
            if !cur.is_empty() {
                paras.push(std::mem::take(&mut cur));
            }
        } else {
            if !cur.is_empty() {
                cur.push(' ');
            }
            cur.push_str(l.trim());
        }
    }
    if !cur.is_empty() {
        paras.push(cur);
    }
    paras.join("\n\n")
}

fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> Result<()> {
    let rd = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) => bail!("cannot list {}: {e}", dir.display()),
    };
    for entry in rd {
        let p = entry?.path();
        if p.is_dir() {
            walk(&p, out)?;
        } else if p.extension().map(|e| e == "proto").unwrap_or(false) {
            out.push(p);
        }
    }
    Ok(())
}

//! The menu interaction state machine, read from the Mermaid diagram that the firmware
//! compiles.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Result};
use regex::Regex;

use crate::util::read_text;

#[derive(Debug, Clone)]
pub struct Transition {
    pub from: String,
    pub to: String,
    pub events: Vec<String>,
    pub guard: Option<String>,
    pub note: Option<String>,
}

pub struct Fsm {
    /// The modes, which are the leaf states, in diagram order.
    pub modes: Vec<String>,
    /// Every transition, with a composite source expanded to each of its leaves.
    pub transitions: Vec<Transition>,
}

impl Fsm {
    pub fn load(path: &Path) -> Result<Fsm> {
        let text = read_text(path)?;
        let body = match text
            .split("```mermaid")
            .nth(1)
            .and_then(|s| s.split("```").next())
        {
            Some(b) => b,
            None => bail!("{} holds no ```mermaid block", path.display()),
        };
        if !body.contains("stateDiagram") {
            bail!(
                "the mermaid block of {} is not a stateDiagram",
                path.display()
            );
        }
        let re_edge =
            Regex::new(r"^\s*(\[\*\]|\w+)\s*-->\s*(\[\*\]|\w+)\s*(?::\s*(.*))?$").unwrap();
        let re_state = Regex::new(r"^\s*state\s+(\w+)\s*\{").unwrap();
        let re_label =
            Regex::new(r"^([A-Za-z_][A-Za-z0-9_ /]*?)\s*(?:\[([^\]]*)\])?\s*(?:\((.*)\))?\s*$")
                .unwrap();

        // composite -> its leaves, in order; composite -> initial child.
        let mut children: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut initial: BTreeMap<String, String> = BTreeMap::new();
        let mut order: Vec<String> = Vec::new();
        let mut raw: Vec<(String, String, String)> = Vec::new();
        let mut stack: Vec<String> = Vec::new();
        let mut top_initial = String::new();

        let note_state = |name: &str,
                          stack: &[String],
                          order: &mut Vec<String>,
                          children: &mut BTreeMap<String, Vec<String>>| {
            if name == "[*]" {
                return;
            }
            if !order.contains(&name.to_string()) {
                order.push(name.to_string());
            }
            if let Some(parent) = stack.last() {
                let v = children.entry(parent.clone()).or_default();
                if !v.contains(&name.to_string()) {
                    v.push(name.to_string());
                }
            }
        };

        for line in body.lines() {
            let t = line.trim();
            if t.is_empty()
                || t.starts_with("%%")
                || t.starts_with("stateDiagram")
                || t.starts_with("note")
            {
                continue;
            }
            if let Some(cap) = re_state.captures(t) {
                let name = cap[1].to_string();
                note_state(&name, &stack, &mut order, &mut children);
                children.entry(name.clone()).or_default();
                stack.push(name);
                continue;
            }
            if t == "}" {
                stack.pop();
                continue;
            }
            if let Some(cap) = re_edge.captures(t) {
                let from = cap[1].to_string();
                let to = cap[2].to_string();
                let label = cap
                    .get(3)
                    .map(|m| m.as_str().trim().to_string())
                    .unwrap_or_default();
                note_state(&from, &stack, &mut order, &mut children);
                note_state(&to, &stack, &mut order, &mut children);
                if from == "[*]" {
                    match stack.last() {
                        Some(parent) => {
                            initial.insert(parent.clone(), to);
                        }
                        None => top_initial = to,
                    }
                    continue;
                }
                if to == "[*]" {
                    continue;
                }
                raw.push((from, to, label));
            }
        }
        let _ = top_initial;

        // A composite with one leaf collapses to its own name.
        let composites: Vec<String> = children
            .iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(k, _)| k.clone())
            .collect();
        let leaves_of = |name: &str| -> Vec<String> {
            match children.get(name) {
                Some(v) if v.len() == 1 => vec![name.to_string()],
                Some(v) if !v.is_empty() => v.clone(),
                _ => vec![name.to_string()],
            }
        };
        let entry_of = |name: &str| -> String {
            match children.get(name) {
                Some(v) if v.len() == 1 => name.to_string(),
                Some(v) if !v.is_empty() => {
                    initial.get(name).cloned().unwrap_or_else(|| v[0].clone())
                }
                _ => name.to_string(),
            }
        };
        let mut modes: Vec<String> = Vec::new();
        for s in &order {
            let is_composite = composites.contains(s);
            let collapsed = children.get(s).map(|v| v.len() == 1).unwrap_or(false);
            let is_child_of_collapsed = children
                .iter()
                .any(|(p, v)| v.len() == 1 && v[0] == *s && composites.contains(p));
            if (!is_composite || collapsed) && !is_child_of_collapsed && !modes.contains(s) {
                modes.push(s.clone());
            }
        }

        let mut transitions = Vec::new();
        for (from, to, label) in raw {
            let (events, guard, note) = match re_label.captures(&label) {
                Some(cap) => (
                    cap[1]
                        .split('/')
                        .map(|e| e.trim().to_string())
                        .filter(|e| !e.is_empty())
                        .collect::<Vec<_>>(),
                    cap.get(2).map(|m| m.as_str().trim().to_string()),
                    cap.get(3).map(|m| m.as_str().trim().to_string()),
                ),
                None => (vec![label.clone()], None, None),
            };
            for f in leaves_of(&from) {
                transitions.push(Transition {
                    from: f,
                    to: entry_of(&to),
                    events: events.clone(),
                    guard: guard.clone(),
                    note: note.clone(),
                });
            }
        }
        if modes.is_empty() || transitions.is_empty() {
            bail!(
                "the diagram of {} has no mode or no transition",
                path.display()
            );
        }
        Ok(Fsm { modes, transitions })
    }

    pub fn from_mode<'a>(&'a self, mode: &str) -> impl Iterator<Item = &'a Transition> + 'a {
        let m = mode.to_string();
        self.transitions.iter().filter(move |t| t.from == m)
    }
}

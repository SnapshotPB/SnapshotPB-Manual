//! The navigation graphic: the menu tree as an SVG that the reader can click. Each box is a
//! screen and links to its section. Each edge is the row that opens the child screen.

use crate::firmware::{Document, ItemType, Placed};
use crate::util::{esc, slug};

const BOX_W: f64 = 168.0;
const GAP_X: f64 = 18.0;
const GAP_Y: f64 = 44.0;
const TITLE_H: f64 = 24.0;
const ROW_H: f64 = 15.0;
const PAD: f64 = 8.0;
const HOME_H: f64 = 34.0;

struct Node<'a> {
    key: &'a str,
    title: &'a str,
    rows: Vec<&'a str>,
    children: Vec<usize>,
    /// The row text that opens this node from its parent.
    via: &'a str,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// The id of the section of a screen.
pub fn screen_id(key: &str) -> String {
    format!("screen-{}", slug(key))
}

/// The graphic of the menu tree, as inline SVG with a link on each screen.
pub fn svg(doc: &Document, placed: &[Placed]) -> String {
    // Build the nodes in walk order; the children of a node come from its submenu rows.
    let mut nodes: Vec<Node> = Vec::with_capacity(placed.len() + 1);
    nodes.push(Node {
        key: "",
        title: &doc.home.title,
        rows: Vec::new(),
        children: Vec::new(),
        via: "",
        x: 0.0,
        y: 0.0,
        w: BOX_W,
        h: HOME_H,
    });
    for p in placed {
        let rows: Vec<&str> = p
            .screen
            .items
            .iter()
            .filter(|i| i.kind == ItemType::Setting)
            .map(|i| i.render.as_str())
            .collect();
        let h = TITLE_H + PAD + rows.len() as f64 * ROW_H;
        nodes.push(Node {
            key: p.key,
            title: &p.screen.title,
            rows,
            children: Vec::new(),
            via: "",
            x: 0.0,
            y: 0.0,
            w: BOX_W,
            h: h.max(HOME_H),
        });
    }
    let index_of = |key: &str| nodes.iter().position(|n| n.key == key && !n.key.is_empty());
    let mut links: Vec<(usize, usize, &str)> = Vec::new();
    for p in placed {
        let parent = index_of(p.key).unwrap_or(0);
        for i in &p.screen.items {
            if i.kind == ItemType::Submenu {
                if let Some(child) = i.target.as_deref().and_then(index_of) {
                    links.push((parent, child, i.render.as_str()));
                }
            }
        }
    }
    if let Some(root) = index_of(&doc.root) {
        links.push((
            0,
            root,
            doc.interactions.get("open_menu").map_or("", |i| {
                doc.controls.get(&i.control).map_or("", |c| c.name.as_str())
            }),
        ));
    }
    for (parent, child, via) in &links {
        nodes[*parent].children.push(*child);
        nodes[*child].via = via;
    }

    // Layout: the width of a subtree is the sum of its children; a parent sits over the
    // middle of its children; a level starts below the tallest box of the level above.
    let widths = subtree_widths(&nodes, 0);
    let mut level_h: Vec<f64> = Vec::new();
    depth_heights(&nodes, 0, 0, &mut level_h);
    let mut level_y = Vec::with_capacity(level_h.len());
    let mut y = PAD;
    for h in &level_h {
        level_y.push(y);
        y += h + GAP_Y;
    }
    place(&mut nodes, &widths, &level_y, 0, 0, PAD);
    let total_w = widths[0] + 2.0 * PAD;
    let total_h = y - GAP_Y + PAD;

    let mut out = String::new();
    out.push_str(&format!(
        "<svg class=\"menu-tree\" viewBox=\"0 0 {total_w:.0} {total_h:.0}\" role=\"img\" aria-label=\"The menu tree. Each box is a screen, and a click on a box opens its section.\">\n"
    ));
    out.push_str("<defs><marker id=\"tree-arrow\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"7\" markerHeight=\"7\" orient=\"auto-start-reverse\"><path d=\"M0 0L10 5L0 10z\" fill=\"currentColor\"/></marker></defs>\n");
    for (parent, child, via) in &links {
        let p = &nodes[*parent];
        let c = &nodes[*child];
        let x1 = p.x + p.w / 2.0;
        let y1 = p.y + p.h;
        let x2 = c.x + c.w / 2.0;
        let y2 = c.y;
        let ym = (y1 + y2) / 2.0;
        out.push_str(&format!(
            "<path class=\"edge\" d=\"M{x1:.1} {y1:.1} V{ym:.1} H{x2:.1} V{y2:.1}\" marker-end=\"url(#tree-arrow)\"/>\n"
        ));
        out.push_str(&format!(
            "<text class=\"edge-label\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"start\">{}</text>\n",
            x2 + 7.0,
            y2 - 7.0,
            esc(via)
        ));
    }
    for n in &nodes {
        let open = if n.key.is_empty() {
            "<g class=\"node node-home\">".to_string()
        } else {
            format!(
                "<a href=\"#{}\" class=\"node\"><title>Open the section {}</title>",
                screen_id(n.key),
                esc(n.title)
            )
        };
        out.push_str(&open);
        out.push_str(&format!(
            "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" rx=\"8\"/>\n",
            n.x, n.y, n.w, n.h
        ));
        out.push_str(&format!(
            "<text class=\"node-title\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\">{}</text>\n",
            n.x + n.w / 2.0,
            n.y + 17.0,
            esc(n.title)
        ));
        for (i, row) in n.rows.iter().enumerate() {
            out.push_str(&format!(
                "<text class=\"node-row\" x=\"{:.1}\" y=\"{:.1}\">{}</text>\n",
                n.x + PAD + 4.0,
                n.y + TITLE_H + PAD + (i as f64 + 0.75) * ROW_H,
                esc(row)
            ));
        }
        out.push_str(if n.key.is_empty() { "</g>\n" } else { "</a>\n" });
    }
    out.push_str("</svg>\n");
    out
}

fn subtree_widths(nodes: &[Node], i: usize) -> Vec<f64> {
    let mut widths = vec![0.0; nodes.len()];
    fill_width(nodes, i, &mut widths);
    widths
}

fn fill_width(nodes: &[Node], i: usize, widths: &mut [f64]) -> f64 {
    let n = &nodes[i];
    let w = if n.children.is_empty() {
        n.w
    } else {
        let sum: f64 = n
            .children
            .iter()
            .map(|c| fill_width(nodes, *c, widths))
            .sum();
        (sum + GAP_X * (n.children.len() as f64 - 1.0)).max(n.w)
    };
    widths[i] = w;
    w
}

fn depth_heights(nodes: &[Node], i: usize, depth: usize, out: &mut Vec<f64>) {
    if out.len() <= depth {
        out.resize(depth + 1, 0.0);
    }
    out[depth] = out[depth].max(nodes[i].h);
    for c in &nodes[i].children {
        depth_heights(nodes, *c, depth + 1, out);
    }
}

fn place(nodes: &mut [Node], widths: &[f64], level_y: &[f64], i: usize, depth: usize, x0: f64) {
    let span = widths[i];
    nodes[i].x = x0 + (span - nodes[i].w) / 2.0;
    nodes[i].y = level_y[depth];
    let children = nodes[i].children.clone();
    let kids: f64 = children.iter().map(|c| widths[*c]).sum::<f64>()
        + GAP_X * (children.len().saturating_sub(1)) as f64;
    let mut x = x0 + (span - kids) / 2.0;
    for c in children {
        place(nodes, widths, level_y, c, depth + 1, x);
        x += widths[c] + GAP_X;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_screen_is_a_box_with_a_link() {
        let doc = Document::example();
        let placed = doc.walk().unwrap();
        let s = svg(&doc, &placed);
        for key in doc.screens.keys() {
            assert!(
                s.contains(&format!("href=\"#screen-{}\"", slug(key))),
                "{key}"
            );
        }
        assert_eq!(s.matches("<rect").count(), doc.screens.len() + 1);
        // The edge from the home screen carries the name of the control that opens the menu.
        assert!(s.contains(">ENTER</text>"));
    }

    #[test]
    fn children_do_not_overlap() {
        let doc = Document::example();
        let placed = doc.walk().unwrap();
        let s = svg(&doc, &placed);
        let re = regex::Regex::new(r#"<rect x="(-?[0-9.]+)" y="(-?[0-9.]+)" width="(-?[0-9.]+)""#)
            .unwrap();
        let boxes: Vec<(f64, f64, f64)> = re
            .captures_iter(&s)
            .map(|c| {
                (
                    c[1].parse().unwrap(),
                    c[2].parse().unwrap(),
                    c[3].parse().unwrap(),
                )
            })
            .collect();
        for (i, a) in boxes.iter().enumerate() {
            assert!(a.0 >= 0.0 && a.1 >= 0.0, "{a:?} is off the canvas");
            for b in boxes.iter().skip(i + 1) {
                if (a.1 - b.1).abs() < 1.0 {
                    assert!(a.0 + a.2 <= b.0 || b.0 + b.2 <= a.0, "{a:?} overlaps {b:?}");
                }
            }
        }
    }
}

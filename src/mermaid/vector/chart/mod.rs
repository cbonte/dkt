//! Flowcharts, state, class and entity-relationship diagrams drawn as vector graphics on the
//! layered layout.

use super::layered::{self, EdgeIn, GroupIn, Input, NodeIn, Outline, P, Seg, Spacing, TitleSide};
use super::text::{self, Anchor, LEADING, SIZE, SMALL, num};
use crate::mermaid::canvas::Weight;
use crate::mermaid::graph::{Direction, Flow, Head, Node, Role, Shape};

mod measure;
mod paint;

pub use measure::{Counted, measure};
use paint::{head_marks, node_text, path, role, shape_svg};

/// The diagram a graph comes from, which sets how its nodes are drawn.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Flowchart,
    State,
    Class,
    Entity,
}

/// Width that a drawing should keep to, so that it reads at its natural size in a page column.
pub const FIT: f64 = 770.0;
/// Margin around the drawing.
const MARGIN: f64 = 6.0;
/// Space between the text of a node and its border, in the roomiest style.
const PAD_X: f64 = 12.0;
const PAD_Y: f64 = 8.0;
/// Narrowest box of a node.
const MIN_W: f64 = 44.0;
/// Room kept beside a node for an edge to itself.
const LOOP: f64 = 30.0;
/// Space between 2 columns of a table in a node.
const COLUMN_GAP: f64 = 12.0;
/// Room for a turn past the side vertex of a fork, the radius of the turn of an edge.
const CORNER: f64 = 10.0;
/// Size of the title of a group.
const TITLE: f64 = 13.0;

/// Spacings and text widths, from the roomiest to the most compact. A drawing wider than its
/// column takes the first style that fits, or the narrowest one and scales down.
struct Style {
    spacing: Spacing,
    /// Widest line of a node and of an edge label before it wraps.
    node_wrap: f64,
    label_wrap: f64,
    /// Space between the text of a node and its border.
    pad_x: f64,
    pad_y: f64,
}

/// A tight spacing, for the 2 most compact styles.
const TIGHT: Spacing = Spacing {
    node: 14.0,
    rank: 28.0,
    edge: 7.0,
    pad: 7.0,
    outside: 10.0,
    turn: CORNER,
};

const STYLES: [Style; 4] = [
    Style {
        spacing: Spacing {
            node: 30.0,
            rank: 44.0,
            edge: 12.0,
            pad: 12.0,
            outside: 18.0,
            turn: CORNER,
        },
        node_wrap: 200.0,
        label_wrap: 160.0,
        pad_x: PAD_X,
        pad_y: PAD_Y,
    },
    Style {
        spacing: Spacing {
            node: 20.0,
            rank: 34.0,
            edge: 9.0,
            pad: 9.0,
            outside: 12.0,
            turn: CORNER,
        },
        node_wrap: 140.0,
        label_wrap: 120.0,
        pad_x: 10.0,
        pad_y: 7.0,
    },
    Style {
        spacing: TIGHT,
        node_wrap: 100.0,
        label_wrap: 90.0,
        pad_x: 7.0,
        pad_y: 6.0,
    },
    Style {
        spacing: TIGHT,
        node_wrap: 72.0,
        label_wrap: 70.0,
        pad_x: 6.0,
        pad_y: 5.0,
    },
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Look {
    Regular,
    Bold,
    Italic,
}

/// A compartment of a node: rows of cells, centred or in columns from the left.
struct Part {
    rows: Vec<(Vec<String>, Look)>,
    centred: bool,
    /// Width of each column.
    columns: Vec<f64>,
}

impl Part {
    fn lines(lines: Vec<String>, look: Look, centred: bool) -> Part {
        let rows: Vec<(Vec<String>, Look)> = lines.into_iter().map(|l| (vec![l], look)).collect();
        Part::new(rows, centred)
    }

    fn new(rows: Vec<(Vec<String>, Look)>, centred: bool) -> Part {
        let count = rows.iter().map(|r| r.0.len()).max().unwrap_or(0);
        let columns = (0..count)
            .map(|c| {
                rows.iter()
                    .filter_map(|(cells, look)| {
                        cells
                            .get(c)
                            .map(|t| text::width(t, SIZE, *look == Look::Bold))
                    })
                    .fold(0.0, f64::max)
            })
            .collect();
        Part {
            rows,
            centred,
            columns,
        }
    }

    fn width(&self) -> f64 {
        let n = self.columns.len();
        self.columns.iter().sum::<f64>() + n.saturating_sub(1) as f64 * COLUMN_GAP
    }

    fn height(&self) -> f64 {
        text::block_height(self.rows.len(), SIZE)
    }
}

/// A node ready to draw: its shape, its size and its text.
struct Shaped {
    shape: Shape,
    w: f64,
    h: f64,
    parts: Vec<Part>,
    /// Space between the text and the border.
    pad_x: f64,
    pad_y: f64,
}

fn shaped(node: &Node, kind: Kind, style: &Style) -> Shaped {
    let (wrap, pad_x, pad_y) = (style.node_wrap, style.pad_x, style.pad_y);
    let shape = node.shape;
    if matches!(shape, Shape::Start | Shape::End) {
        let d = if shape == Shape::Start { 14.0 } else { 18.0 };
        return Shaped {
            shape,
            w: d,
            h: d,
            parts: Vec::new(),
            pad_x,
            pad_y,
        };
    }
    let sections = sections(node);
    let parts: Vec<Part> = match kind {
        Kind::Flowchart => {
            let lines = text::wrap(&node.lines, SIZE, false, wrap);
            vec![Part::lines(lines, Look::Regular, true)]
        }
        Kind::State => sections
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let lines = text::wrap(s, SIZE, false, wrap);
                Part::lines(lines, if i == 0 { Look::Bold } else { Look::Regular }, true)
            })
            .collect(),
        Kind::Class => sections
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if i == 0 {
                    let rows = s
                        .iter()
                        .map(|l| {
                            let look = if l.starts_with("<<") {
                                Look::Italic
                            } else {
                                Look::Bold
                            };
                            (vec![l.clone()], look)
                        })
                        .collect();
                    Part::new(rows, true)
                } else {
                    Part::lines(s.clone(), Look::Regular, false)
                }
            })
            .collect(),
        Kind::Entity => sections
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if i == 0 {
                    Part::lines(s.clone(), Look::Bold, true)
                } else {
                    let rows = s
                        .iter()
                        .map(|l| {
                            (
                                l.split_whitespace().map(str::to_string).collect(),
                                Look::Regular,
                            )
                        })
                        .collect();
                    Part::new(rows, false)
                }
            })
            .collect(),
    };
    let tw = parts.iter().map(Part::width).fold(0.0, f64::max);
    let parts_h: f64 = parts.iter().map(Part::height).sum();
    let th = parts_h + (parts.len().saturating_sub(1)) as f64 * 2.0 * pad_y;
    let base_h = th + 2.0 * pad_y;
    let (w, h) = match shape {
        Shape::Diamond => {
            let a = tw / 2.0 + th * 1.5 + 4.0;
            let b = (th / 2.0) / (1.0 - (tw / 2.0) / a) + 4.0;
            (2.0 * a, 2.0 * b)
        }
        Shape::Circle | Shape::DoubleCircle => {
            let d = (tw * tw + th * th).sqrt() + 12.0;
            let d = d.max(40.0)
                + if shape == Shape::DoubleCircle {
                    8.0
                } else {
                    0.0
                };
            (d, d)
        }
        Shape::Stadium => (tw + base_h + 4.0, base_h),
        Shape::Hexagon => (tw + 2.0 * pad_x + base_h / 2.0, base_h),
        Shape::Asymmetric => (tw + 2.0 * pad_x + base_h / 3.0, base_h),
        Shape::Lean(_) | Shape::Trapezoid(_) => (tw + 2.0 * pad_x + base_h * 0.7, base_h),
        Shape::Subroutine => (tw + 2.0 * pad_x + 14.0, base_h),
        Shape::Cylinder => {
            let w = (tw + 2.0 * pad_x).max(MIN_W);
            (w, base_h + 2.0 * cylinder_ry(w))
        }
        _ => (tw + 2.0 * pad_x, base_h),
    };
    Shaped {
        shape,
        w: w.max(MIN_W),
        h,
        parts,
        pad_x,
        pad_y,
    }
}

fn cylinder_ry(w: f64) -> f64 {
    (w * 0.08).clamp(4.0, 9.0)
}

/// The lines of a node split at its dividers.
fn sections(node: &Node) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    let mut start = 0;
    for &d in node
        .dividers
        .iter()
        .chain(std::iter::once(&node.lines.len()))
    {
        if d >= start && d <= node.lines.len() {
            out.push(node.lines[start..d].to_vec());
            start = d;
        }
    }
    out.retain(|s| !s.is_empty());
    if out.is_empty() {
        out.push(Vec::new());
    }
    out
}

/// Whether a node of `shape` is a box of text, which shares its length along the flow with the
/// boxes of its rank.
fn boxy(shape: Shape) -> bool {
    !matches!(
        shape,
        Shape::Diamond | Shape::Circle | Shape::DoubleCircle | Shape::Start | Shape::End
    )
}

/// How the outline of a node of `s` meets an edge that arrives along the flow.
fn outline(s: &Shaped, horizontal: bool) -> Outline {
    match s.shape {
        Shape::Diamond => Outline::Diamond,
        Shape::Circle | Shape::DoubleCircle | Shape::Start | Shape::End | Shape::Stadium => {
            Outline::Round
        }
        Shape::Hexagon if horizontal => Outline::Hexagon(s.h / 4.0),
        Shape::Cylinder if !horizontal => Outline::Cylinder(cylinder_ry(s.w)),
        Shape::Lean(_) | Shape::Trapezoid(_) if !horizontal => Outline::Narrow(s.h * 0.35),
        _ => Outline::Rect,
    }
}

/// A drawing: its elements and its size.
struct Drawing {
    body: String,
    w: f64,
    h: f64,
    /// The path of each edge between 2 nodes, on the page.
    paths: Vec<Vec<Seg>>,
    /// The centre, width and height of the 2 nodes of each of those edges.
    ends: Vec<[(P, f64, f64); 2]>,
    /// Each node: its centre, its width and height, and whether it is a box of text.
    boxes: Vec<(P, f64, f64, bool)>,
}

/// Maps the space of the ranks to the page.
struct Frame {
    direction: Direction,
}

impl Frame {
    fn to_page(&self, (a, l): P) -> P {
        match self.direction {
            Direction::TopDown => (a, l),
            Direction::BottomUp => (a, -l),
            Direction::LeftRight => (l, a),
            Direction::RightLeft => (-l, a),
        }
    }

    fn seg(&self, s: Seg) -> Seg {
        match s {
            Seg::Move(p) => Seg::Move(self.to_page(p)),
            Seg::Line(p) => Seg::Line(self.to_page(p)),
            Seg::Curve(a, b, p) => Seg::Curve(self.to_page(a), self.to_page(b), self.to_page(p)),
        }
    }
}

/// Draws `flow` as an SVG element that holds `source`. `None` when the layout fails.
pub fn svg(flow: &Flow, kind: Kind, source: &str) -> Option<String> {
    let d = choose(flow, kind)?;
    Some(format!("{}{}</svg>", text::open(d.w, d.h, source), d.body))
}

/// The drawing of `flow` that fits its column, in the direction of the source.
fn choose(flow: &Flow, kind: Kind) -> Option<Drawing> {
    // The direction of the source is kept. The first style that fits the column wins, else the
    // narrowest drawing, which the page scales down.
    let mut best: Option<Drawing> = None;
    for style in &STYLES {
        let drawing = draw(flow, kind, style, flow.direction)?;
        let fits = drawing.w <= FIT;
        if best.as_ref().is_none_or(|b| drawing.w < b.w) {
            best = Some(drawing);
        }
        if fits {
            break;
        }
    }
    best
}

/// Unit vector from `a` to `b`.
fn unit(a: P, b: P) -> P {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-9 {
        (0.0, 1.0)
    } else {
        (dx / len, dy / len)
    }
}

fn end_point(s: &Seg) -> P {
    match *s {
        Seg::Move(p) | Seg::Line(p) | Seg::Curve(_, _, p) => p,
    }
}

/// Length that a head takes from the end of its edge.
fn head_length(head: Head) -> f64 {
    match head {
        Head::None | Head::One | Head::OneMany => 0.0,
        Head::Arrow => 8.0,
        Head::Circle => 9.0,
        Head::Cross => 5.0,
        Head::Inherit => 13.0,
        Head::Composition | Head::Aggregation => 17.0,
        Head::ZeroOne => 18.0,
        Head::ZeroMany => 19.0,
    }
}

/// The tip and the heading of the end of `segs`, at the last point or at the first.
fn tip(segs: &[Seg], last: bool) -> Option<(P, P)> {
    if last {
        let n = segs.len();
        let end = end_point(segs.last()?);
        let from = match segs[n - 1] {
            Seg::Curve(c1, c2, p) => {
                if (c2.0 - p.0).abs() + (c2.1 - p.1).abs() > 1e-6 {
                    c2
                } else {
                    c1
                }
            }
            _ => end_point(segs.get(n.checked_sub(2)?)?),
        };
        Some((end, unit(from, end)))
    } else {
        let start = end_point(segs.first()?);
        let next = match *segs.get(1)? {
            Seg::Curve(c1, _, _) => c1,
            s => end_point(&s),
        };
        Some((start, unit(next, start)))
    }
}

/// Pulls the end of `segs` back by `len` along its heading.
fn shorten(segs: &mut [Seg], last: bool, len: f64) {
    if len <= 0.0 || segs.len() < 2 {
        return;
    }
    let Some((_, d)) = tip(segs, last) else {
        return;
    };
    let back = |p: P| (p.0 - d.0 * len, p.1 - d.1 * len);
    if last {
        let n = segs.len();
        segs[n - 1] = match segs[n - 1] {
            Seg::Line(p) => Seg::Line(back(p)),
            Seg::Curve(a, b, p) => Seg::Curve(a, back(b), back(p)),
            s => s,
        };
    } else {
        if let Seg::Move(p) = segs[0] {
            segs[0] = Seg::Move(back(p));
        }
        if let Seg::Curve(a, b, p) = segs[1] {
            segs[1] = Seg::Curve(back(a), b, p);
        }
    }
}

/// Points along `segs`, the curves cut into short lines.
fn sample(segs: &[Seg]) -> Vec<Vec<P>> {
    let mut runs: Vec<Vec<P>> = Vec::new();
    let mut at = (0.0, 0.0);
    for s in segs {
        match *s {
            Seg::Move(p) => {
                runs.push(vec![p]);
                at = p;
            }
            Seg::Line(p) => {
                if let Some(r) = runs.last_mut() {
                    r.push(p);
                }
                at = p;
            }
            Seg::Curve(a, b, p) => {
                if let Some(r) = runs.last_mut() {
                    for k in 1..=8 {
                        let t = f64::from(k) / 8.0;
                        let u = 1.0 - t;
                        let x = u * u * u * at.0
                            + 3.0 * u * u * t * a.0
                            + 3.0 * u * t * t * b.0
                            + t * t * t * p.0;
                        let y = u * u * u * at.1
                            + 3.0 * u * u * t * a.1
                            + 3.0 * u * t * t * b.1
                            + t * t * t * p.1;
                        r.push((x, y));
                    }
                }
                at = p;
            }
        }
    }
    runs
}

/// Whether a line from `a` to `b` crosses the box `r`: left, top, right, bottom.
fn crosses(a: P, b: P, r: [f64; 4]) -> bool {
    let steps = 12;
    (0..=steps).any(|k| {
        let t = f64::from(k) / f64::from(steps);
        let (x, y) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        x > r[0] && x < r[2] && y > r[1] && y < r[3]
    })
}

fn draw(flow: &Flow, kind: Kind, style: &Style, direction: Direction) -> Option<Drawing> {
    let horizontal = direction.horizontal();
    let frame = Frame { direction };
    let mut shapes: Vec<Shaped> = flow.nodes.iter().map(|n| shaped(n, kind, style)).collect();
    // A side that may hold n ports, a gap from each other and from the corners, is at least
    // (n + 1) gaps long: the edges in and the edges out of a node across the flow.
    let mut ins = vec![0usize; flow.nodes.len()];
    let mut outs = vec![0usize; flow.nodes.len()];
    for e in flow.edges.iter().filter(|e| e.from != e.to) {
        outs[e.from] += 1;
        ins[e.to] += 1;
    }
    for (v, s) in shapes.iter_mut().enumerate() {
        if s.parts.is_empty() {
            continue;
        }
        let need = (ins[v].max(outs[v]) + 1) as f64 * layered::MIN_GAP;
        if horizontal {
            s.h = s.h.max(need);
        } else {
            s.w = s.w.max(need);
        }
    }
    let label_lines: Vec<Option<Vec<String>>> = flow
        .edges
        .iter()
        .map(|e| {
            let lines: Vec<String> = e.label.as_ref()?.split('\n').map(str::to_string).collect();
            Some(text::wrap(&lines, SMALL, false, style.label_wrap))
        })
        .collect();
    let label_size = |lines: &Vec<String>| {
        (
            text::block_width(lines, SMALL, false) + 8.0,
            text::block_height(lines.len(), SMALL) + 2.0,
        )
    };
    // Room for the edges from a node to itself, beside it across the flow.
    let mut loop_room = vec![0.0f64; flow.nodes.len()];
    for (e, edge) in flow.edges.iter().enumerate() {
        if edge.from == edge.to {
            let label = label_lines[e].as_ref().map_or((0.0, 0.0), label_size);
            let room = LOOP + if horizontal { label.1 } else { label.0 + 4.0 };
            loop_room[edge.from] = loop_room[edge.from].max(room);
        }
    }
    let nodes: Vec<NodeIn> = shapes
        .iter()
        .zip(&loop_room)
        .map(|(s, &room)| {
            let (across, along) = if horizontal { (s.h, s.w) } else { (s.w, s.h) };
            NodeIn {
                lo: across / 2.0,
                hi: across / 2.0 + room,
                along,
                outline: outline(s, horizontal),
                boxy: boxy(s.shape),
            }
        })
        .collect();
    let laid: Vec<usize> = (0..flow.edges.len())
        .filter(|&e| flow.edges[e].from != flow.edges[e].to)
        .collect();
    let edges: Vec<EdgeIn> = laid
        .iter()
        .map(|&e| {
            let edge = &flow.edges[e];
            let label = label_lines[e].as_ref().map(|l| {
                let (w, h) = label_size(l);
                if horizontal { (h, w) } else { (w, h) }
            });
            EdgeIn {
                from: edge.from,
                to: edge.to,
                label,
                heads: (head_length(edge.from_end), head_length(edge.to_end)),
                look: edge.look(&flow.edges),
            }
        })
        .collect();
    let titles: Vec<Vec<String>> = flow
        .groups
        .iter()
        .map(|g| text::wrap(&g.title, TITLE, true, style.node_wrap * 1.5))
        .collect();
    let groups: Vec<GroupIn> = flow
        .groups
        .iter()
        .zip(&titles)
        .map(|(g, t)| {
            let (w, h) = (
                text::block_width(t, TITLE, true),
                text::block_height(t.len(), TITLE),
            );
            GroupIn {
                parent: g.parent,
                nodes: g.nodes.clone(),
                title: if horizontal { (h, w) } else { (w, h) },
            }
        })
        .collect();
    let title_side = match direction {
        Direction::TopDown => TitleSide::AlongLow,
        Direction::BottomUp => TitleSide::AlongHigh,
        _ => TitleSide::AcrossLow,
    };
    let input = Input {
        under_parents: true,
        nodes,
        edges,
        groups,
        title_side,
        spacing: style.spacing,
    };
    let out = layered::layout(&input)?;
    // A node that grew to hold the edges on its sides, or to merge edges under its parents.
    for ((s, &along), &lo) in shapes.iter_mut().zip(&out.along).zip(&out.lo) {
        if horizontal {
            s.w = s.w.max(along);
            s.h = s.h.max(2.0 * lo);
        } else {
            s.h = s.h.max(along);
            s.w = s.w.max(2.0 * lo);
        }
    }

    // Everything on the page, before the margin is added.
    let centres: Vec<P> = out.nodes.iter().map(|&p| frame.to_page(p)).collect();
    let mut paths: Vec<(usize, Vec<Seg>)> = Vec::new();
    let mut main = vec![false; flow.edges.len()];
    for (k, &e) in laid.iter().enumerate() {
        main[e] = out.main.get(k).copied().unwrap_or(false);
        let segs: Vec<Seg> = out.edges[k].iter().map(|&s| frame.seg(s)).collect();
        paths.push((e, segs));
    }
    for (e, edge) in flow.edges.iter().enumerate() {
        if edge.from != edge.to {
            continue;
        }
        let c = centres[edge.from];
        let s = &shapes[edge.from];
        let segs = if horizontal {
            let y = c.1 + s.h / 2.0;
            vec![
                Seg::Move((c.0 - 7.0, y)),
                Seg::Curve(
                    (c.0 - 24.0, y + 30.0),
                    (c.0 + 24.0, y + 30.0),
                    (c.0 + 7.0, y),
                ),
            ]
        } else {
            let reach = |dy: f64| c.0 + layered::reach(outline(s, true), s.h, s.w, dy);
            vec![
                Seg::Move((reach(-7.0), c.1 - 7.0)),
                Seg::Curve(
                    (c.0 + s.w / 2.0 + 30.0, c.1 - 24.0),
                    (c.0 + s.w / 2.0 + 30.0, c.1 + 24.0),
                    (reach(7.0), c.1 + 7.0),
                ),
            ]
        };
        paths.push((e, segs));
    }
    paths.sort_by_key(|p| p.0);
    let between: Vec<Vec<Seg>> = paths
        .iter()
        .filter(|(e, _)| flow.edges[*e].from != flow.edges[*e].to)
        .map(|(_, s)| s.clone())
        .collect();
    let ends: Vec<[(P, f64, f64); 2]> = paths
        .iter()
        .filter(|(e, _)| flow.edges[*e].from != flow.edges[*e].to)
        .map(|(e, _)| {
            let node = |v: usize| (centres[v], shapes[v].w, shapes[v].h);
            [node(flow.edges[*e].from), node(flow.edges[*e].to)]
        })
        .collect();
    let mut labels: Vec<Option<P>> = vec![None; flow.edges.len()];
    for (k, &e) in laid.iter().enumerate() {
        labels[e] = out.labels[k].map(|p| frame.to_page(p));
    }
    for (e, edge) in flow.edges.iter().enumerate() {
        if edge.from == edge.to
            && let Some(lines) = &label_lines[e]
        {
            let c = centres[edge.from];
            let s = &shapes[edge.from];
            let (w, h) = label_size(lines);
            labels[e] = Some(if horizontal {
                (c.0, c.1 + s.h / 2.0 + 26.0 + h / 2.0)
            } else {
                (c.0 + s.w / 2.0 + 28.0 + w / 2.0, c.1)
            });
        }
    }
    let boxes: Vec<Option<[f64; 4]>> = out
        .groups
        .iter()
        .map(|b| {
            let b = (*b)?;
            let (p, q) = (frame.to_page((b[0], b[1])), frame.to_page((b[2], b[3])));
            Some([p.0.min(q.0), p.1.min(q.1), p.0.max(q.0), p.1.max(q.1)])
        })
        .collect();

    // Bounds.
    let (mut x0, mut y0, mut x1, mut y1) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    let mut grow = |x: f64, y: f64| {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    };
    for (c, s) in centres.iter().zip(&shapes) {
        grow(c.0 - s.w / 2.0, c.1 - s.h / 2.0);
        grow(c.0 + s.w / 2.0, c.1 + s.h / 2.0);
    }
    for b in boxes.iter().flatten() {
        grow(b[0], b[1]);
        grow(b[2], b[3]);
    }
    for (_, segs) in &paths {
        for run in sample(segs) {
            for p in run {
                grow(p.0, p.1);
            }
        }
    }
    for (e, l) in labels.iter().enumerate() {
        if let (Some(c), Some(lines)) = (l, &label_lines[e]) {
            let (w, h) = label_size(lines);
            grow(c.0 - w / 2.0, c.1 - h / 2.0);
            grow(c.0 + w / 2.0, c.1 + h / 2.0);
        }
    }
    if !(x0.is_finite() && y0.is_finite()) {
        return None;
    }
    let (dx, dy) = (MARGIN - x0, MARGIN - y0);
    let (w, h) = (x1 - x0 + 2.0 * MARGIN, y1 - y0 + 2.0 * MARGIN);

    let mut body = String::new();
    // Group boxes, outer first, with their titles where no edge runs.
    let mut titles_svg = String::new();
    let samples: Vec<Vec<P>> = paths.iter().flat_map(|(_, s)| sample(s)).collect();
    if boxes.iter().any(Option::is_some) {
        body.push_str(
            "<g class=\"dkt-tint dkt-group\" fill=\"currentColor\" fill-opacity=\"0.035\" stroke-opacity=\"0.5\">",
        );
        for (g, b) in boxes.iter().enumerate() {
            let Some(b) = b else { continue };
            body.push_str(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"6\"/>",
                num(b[0] + dx),
                num(b[1] + dy),
                num(b[2] - b[0]),
                num(b[3] - b[1])
            ));
            let lines = &titles[g];
            let tw = text::block_width(lines, TITLE, true);
            let th = text::block_height(lines.len(), TITLE);
            let pad = style.spacing.pad;
            let top = b[1] + 5.0;
            let candidates = [
                (b[0] + pad, Anchor::Start, b[0] + pad),
                (b[2] - pad - tw, Anchor::Start, b[2] - pad - tw),
                (
                    (b[0] + b[2]) / 2.0 - tw / 2.0,
                    Anchor::Start,
                    (b[0] + b[2]) / 2.0 - tw / 2.0,
                ),
            ];
            let clear = |left: f64| {
                let r = [left - 3.0, top - 2.0, left + tw + 3.0, top + th + 2.0];
                !samples
                    .iter()
                    .any(|run| run.windows(2).any(|s| crosses(s[0], s[1], r)))
            };
            let (x, anchor, _) = candidates
                .iter()
                .copied()
                .find(|c| clear(c.2))
                .unwrap_or(candidates[0]);
            titles_svg.push_str(&text::text(
                lines,
                x + dx,
                top + th / 2.0 + dy,
                TITLE,
                anchor,
                " font-weight=\"bold\"",
            ));
        }
        body.push_str("</g>");
    }
    // Edges by weight, the main path apart, then their heads.
    let mut strokes: [[String; 2]; 3] = Default::default();
    let mut heads: [String; 2] = Default::default();
    for (e, segs) in &mut paths {
        let edge = &flow.edges[*e];
        let start = tip(segs, false);
        let end = tip(segs, true);
        shorten(segs, true, head_length(edge.to_end));
        shorten(segs, false, head_length(edge.from_end));
        let slot = match edge.weight {
            Weight::Normal => 0,
            Weight::Thick => 1,
            Weight::Dotted => 2,
        };
        let spine = usize::from(main[*e]);
        strokes[slot][spine].push_str(&path(segs, dx, dy));
        let moved = |(p, d): (P, P)| ((p.0 + dx, p.1 + dy), d);
        if let Some((p, d)) = end.map(moved) {
            heads[spine].push_str(&head_marks(edge.to_end, p, d));
        }
        if let Some((p, d)) = start.map(moved) {
            heads[spine].push_str(&head_marks(edge.from_end, p, d));
        }
    }
    // The main path is drawn a little wider, in the accent colour.
    let widths = [["1.3", "1.7"], ["2.6", "3"], ["1.3", "1.7"]];
    let classes = ["", " class=\"dkt-line dkt-accent\""];
    for (slot, pair) in strokes.iter().enumerate() {
        for (spine, d) in pair.iter().enumerate() {
            if d.is_empty() {
                continue;
            }
            let dash = if slot == 2 {
                " stroke-dasharray=\"4 3\""
            } else {
                ""
            };
            body.push_str(&format!(
                "<path d=\"{d}\"{} stroke-width=\"{}\"{dash} stroke-linecap=\"round\" stroke-linejoin=\"round\"/>",
                classes[spine], widths[slot][spine]
            ));
        }
    }
    let head_classes = ["", " class=\"dkt-head dkt-accent\""];
    for (spine, h) in heads.iter().enumerate() {
        if !h.is_empty() {
            body.push_str(&format!(
                "<g{} stroke-width=\"1.3\" stroke-linejoin=\"round\">{h}</g>",
                head_classes[spine]
            ));
        }
    }
    // Nodes, then all the text.
    body.push_str("<g fill=\"currentColor\" fill-opacity=\"0.08\" stroke-width=\"1.3\">");
    let mut words = String::new();
    let mut rules = String::new();
    for (c, s) in centres.iter().zip(&shapes) {
        body.push_str(&format!(
            "<g class=\"dkt-fill {}\">{}</g>",
            role(s.shape),
            shape_svg(s, *c, dx, dy, kind)
        ));
        let (t, r) = node_text(s, *c, dx, dy);
        words.push_str(&t);
        rules.push_str(&r);
    }
    body.push_str("</g>");
    if !rules.is_empty() {
        body.push_str(&format!("<path d=\"{rules}\" stroke-width=\"1\"/>"));
    }
    // A tinted pill under each label of an edge.
    let mut pills = String::new();
    for (e, l) in labels.iter().enumerate() {
        if let (Some(c), Some(lines)) = (l, &label_lines[e]) {
            let (w, h) = label_size(lines);
            pills.push_str(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\"/>",
                num(c.0 + dx - w / 2.0),
                num(c.1 + dy - h / 2.0),
                num(w),
                num(h),
                num(h.min(w) / 2.0)
            ));
        }
    }
    if !pills.is_empty() {
        body.push_str(&format!(
            "<g class=\"dkt-pill dkt-accent\" stroke=\"none\">{pills}</g>"
        ));
    }
    for (e, l) in labels.iter().enumerate() {
        if let (Some(c), Some(lines)) = (l, &label_lines[e]) {
            words.push_str(&text::text(
                lines,
                c.0 + dx,
                c.1 + dy,
                SMALL,
                Anchor::Middle,
                "",
            ));
        }
    }
    body.push_str(&format!(
        "<g fill=\"currentColor\" stroke=\"none\">{titles_svg}{words}</g>"
    ));
    Some(Drawing {
        body,
        w,
        h,
        paths: between,
        ends,
        boxes: centres
            .iter()
            .zip(&shapes)
            .map(|(&c, s)| (c, s.w, s.h, boxy(s.shape)))
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::parse::parse;

    #[test]
    fn the_chain_of_rules_has_no_elbow() {
        let flow = parse("flowchart TD\n  call[\"A call that changes something\"] --> pre{\"The review model, planning or a pre-tool hook refuses?\"}\n  pre -->|no| deny{\"deny rule?\"}\n  deny -->|no| ask{\"ask rule?\"}\n  ask -->|no| allow{\"allow rule, under max-auto?\"}\n  allow -->|no| agreed{\"Writing without asking, or an agreement?\"}\n  agreed -->|no| you[\"You answer\"]\n  pre --> back[\"Refused, back to the model with why\"]\n  deny --> back\n  ask --> you\n  allow --> run[\"It runs\"]\n  agreed --> run\n  you -->|\"y or a\"| run\n  you -->|\"n, e or x\"| back\n").unwrap();
        let d = choose(&flow, Kind::Flowchart).unwrap();
        // pre to deny, deny to ask, ask to allow, allow to agreed.
        for e in 1..5 {
            assert!(
                d.paths[e].iter().all(|s| !matches!(s, Seg::Curve(..))),
                "edge {e}: {:?}",
                d.paths[e]
            );
        }
    }

    #[test]
    fn a_chain_has_no_bend() {
        let flow =
            parse("graph LR\n A[One] --> B[Two words] --> C[x] --> D[The last step]\n").unwrap();
        let counted = measure(&flow, Kind::Flowchart).unwrap();
        assert_eq!((counted.crossings, counted.bends), (0, 0));
    }

    /// The point where each end of each edge meets its node, with the node: centre, width,
    /// height, and whether the edge meets it on a side across the flow.
    fn meets(d: &Drawing) -> Vec<(P, P, f64, f64, bool)> {
        let mut out = Vec::new();
        for (segs, ends) in d.paths.iter().zip(&d.ends) {
            let first = end_point(&segs[0]);
            let next = match segs.get(1) {
                Some(Seg::Curve(c1, _, _)) => *c1,
                Some(s) => end_point(s),
                None => first,
            };
            let last = end_point(&segs[segs.len() - 1]);
            let before = match segs[segs.len() - 1] {
                Seg::Curve(_, c2, _) => c2,
                _ => end_point(&segs[segs.len() - 2]),
            };
            let (c0, w0, h0) = ends[0];
            let (c1, w1, h1) = ends[1];
            out.push((first, c0, w0, h0, (next.1 - first.1).abs() < 1e-6));
            out.push((last, c1, w1, h1, (before.1 - last.1).abs() < 1e-6));
        }
        out
    }

    #[test]
    fn the_rules_cross_nothing() {
        let flow = parse("flowchart TD\n  call[\"A call that changes something\"] --> pre{\"The review model, planning or a pre-tool hook refuses?\"}\n  pre -->|no| deny{\"deny rule?\"}\n  deny -->|no| ask{\"ask rule?\"}\n  ask -->|no| allow{\"allow rule, under max-auto?\"}\n  allow -->|no| agreed{\"Writing without asking, or an agreement?\"}\n  agreed -->|no| you[\"You answer\"]\n  pre --> back[\"Refused, back to the model with why\"]\n  deny --> back\n  ask --> you\n  allow --> run[\"It runs\"]\n  agreed --> run\n  you -->|\"y or a\"| run\n  you -->|\"n, e or x\"| back\n").unwrap();
        assert_eq!(measure(&flow, Kind::Flowchart).unwrap().crossings, 0);
    }

    #[test]
    fn an_edge_alone_on_a_side_meets_its_middle() {
        let flow = parse("flowchart TD\n  start[\"An agent with background that changes files\"] --> copy{\"A copy can be made?\"}\n  copy -->|yes| work[\"It works in its copy, its changes asked there\"]\n  copy -->|no| hold[\"It writes in the project, holding its files\"]\n  work --> ans[\"Its answer, with the patch and a number\"]\n  ans --> take{\"take_work: did the project change?\"}\n  take -->|no| inn[\"The work comes in\"]\n  take -->|yes| show[\"Nothing written: both sides of each shared file\"]\n  show --> merge[\"The session merges, calls again with proofs\"]\n  merge --> proof{\"Every added and removed line accounted for?\"}\n  proof -->|\"no: the lines are listed\"| merge\n  proof -->|yes| coh[\"The rest comes in, a model reads both sides\"]\n  inn --> after[\"Checked like any write, one change for /undo\"]\n  coh --> after\n").unwrap();
        let d = choose(&flow, Kind::Flowchart).unwrap();
        let all = meets(&d);
        // The side of each end: a side across the flow by the sign of the offset across, else
        // the top or the bottom.
        let side = |m: &(P, P, f64, f64, bool)| {
            let (p, c, _, _, across) = *m;
            let key = |v: f64| (v * 10.0).round() as i64;
            if across {
                (key(c.0), key(c.1), if p.0 < c.0 { 'w' } else { 'e' })
            } else {
                (key(c.0), key(c.1), if p.1 < c.1 { 'n' } else { 's' })
            }
        };
        // A lone end off the middle only keeps an edge straight, which comes first.
        let straight: Vec<bool> = d
            .paths
            .iter()
            .flat_map(|p| {
                let s = p.iter().all(|s| !matches!(s, Seg::Curve(..)));
                [s, s]
            })
            .collect();
        for (m, &line) in all.iter().zip(&straight) {
            let lone = all.iter().filter(|o| side(o) == side(m)).count() == 1;
            let (p, c, _, _, across) = *m;
            let middle = if across {
                (p.1 - c.1).abs() < 0.5
            } else {
                (p.0 - c.0).abs() < 0.5
            };
            assert!(!lone || middle || line, "{p:?} on the node at {c:?}");
        }
    }

    #[test]
    fn a_decision_goes_on_down_and_branches_across() {
        // The edge toward the longer chain carries on down from the bottom vertex, straight; the
        // other leaf stands beside the diamond, reached across from a side vertex.
        let flow = parse(
            "graph TD\n D{Go?} -->|yes| A[Done]\n D -->|no| B[Next step]\n B --> C[Last step]\n",
        )
        .unwrap();
        let d = choose(&flow, Kind::Flowchart).unwrap();
        let ends = meets(&d);
        // yes: from the side vertex, horizontal, to a node in the same row.
        let (p, c, w, _, across) = ends[0];
        assert!(across && (p.1 - c.1).abs() < 0.5 && ((p.0 - c.0).abs() - w / 2.0).abs() < 0.5);
        let (q, t, _, _, _) = ends[1];
        assert!(
            (q.1 - p.1).abs() < 0.5 && (t.1 - c.1).abs() < 0.5,
            "{q:?} {t:?}"
        );
        // no: from the bottom vertex, straight down to the node under it.
        let (p, c, _, h, across) = ends[2];
        assert!(!across && (p.0 - c.0).abs() < 0.5 && ((p.1 - c.1) - h / 2.0).abs() < 0.5);
        let (q, t, _, _, _) = ends[3];
        assert!(
            (q.0 - p.0).abs() < 0.5 && (t.0 - c.0).abs() < 0.5,
            "{q:?} {t:?}"
        );
    }

    #[test]
    fn a_single_successor_sits_under_its_parent() {
        let flow =
            parse("graph TD\n A[A rather long first step] --> B[Go]\n B --> C[x]\n").unwrap();
        let d = choose(&flow, Kind::Flowchart).unwrap();
        for (a, b) in d.ends.iter().map(|e| (e[0].0, e[1].0)) {
            assert!((a.0 - b.0).abs() < 0.5, "{a:?} {b:?}");
        }
    }

    #[test]
    fn a_merge_off_the_main_path_has_no_step() {
        // The main path runs R, P, Q, W; A and B branch off it and meet again in M. Each edge
        // into M runs straight or turns once, by a side of M.
        let flow = parse(
            "graph TD\n R --> A[Left] & B[Right] & P\n P --> Q --> W\n A --> M[Merge]\n B --> M\n",
        )
        .unwrap();
        let d = choose(&flow, Kind::Flowchart).unwrap();
        for k in [5usize, 6] {
            let mut at = (0.0, 0.0);
            for s in &d.paths[k] {
                if let Seg::Curve(c1, c2, q) = *s {
                    let (a, b) = ((c1.0 - at.0, c1.1 - at.1), (q.0 - c2.0, q.1 - c2.1));
                    let step = (q.0 - at.0).abs() > 0.5
                        && (q.1 - at.1).abs() > 0.5
                        && (a.0 * b.1 - a.1 * b.0).abs() < 1e-6;
                    assert!(!step, "edge {k}: {:?}", d.paths[k]);
                }
                at = end_point(s);
            }
        }
    }

    #[test]
    fn the_main_path_runs_straight() {
        // R branches; the edge toward the longest chain carries the main path, straight down
        // through every node of it, a merge included.
        let flow =
            parse("graph TD\n R --> A[Side] & B[Main]\n B --> C[Next] --> M[Merge]\n A --> M\n")
                .unwrap();
        let d = choose(&flow, Kind::Flowchart).unwrap();
        let main = [1usize, 2, 3];
        for k in main {
            let (a, b) = (d.ends[k][0].0, d.ends[k][1].0);
            let p = &d.paths[k];
            let (s, e) = (end_point(&p[0]), end_point(&p[p.len() - 1]));
            assert!(
                (s.0 - e.0).abs() < 0.5,
                "edge {k}: {p:?} from {a:?} to {b:?}"
            );
        }
    }

    #[test]
    fn arrows_on_a_wide_side_run_straight() {
        // A fork of 3 meeting again in a target wide enough for the 3 to arrive from above.
        let flow = parse(
            "graph TD\n R[Start] --> A[one] & B[two] & C[three]\n \
             A --> T[Atargetwiderthanits3parentstogetherareveryverywide]\n B --> T\n C --> T\n",
        )
        .unwrap();
        let d = choose(&flow, Kind::Flowchart).unwrap();
        for k in 3..6 {
            let p = &d.paths[k];
            let (s, e) = (end_point(&p[0]), end_point(&p[p.len() - 1]));
            assert!((s.0 - e.0).abs() < 0.5, "edge {k}: {p:?}");
        }
    }

    #[test]
    fn an_edge_is_square_or_one_diagonal() {
        let charts = [
            "graph TD\n R[Start] --> A[one] & B[two] & C[three] & D[four] & E[five]\n A --> F[x]\n E --> F\n C --> G[y]\n",
            "flowchart TD\n  model([\"The model\"]) -->|\"a call\"| gate{\"Approval\"}\n  you([\"You\"]) -->|\"answers\"| gate\n  gate -->|\"writes\"| files[\"File tools\"]\n  gate --> fetch[\"web_fetch\"]\n  gate --> cmd[\"bash\"]\n  gate --> out[\"outside\"]\n  gate --> mcp[\"Tool servers\"]\n  files --> store[(\"Transcript\")]\n  files -.-> progs[\"Hooks\"]\n",
            "graph LR\n A --> B & C & D\n B --> E\n C --> E\n D --> A\n",
        ];
        for text in charts {
            let flow = parse(text).unwrap();
            assert_eq!(measure(&flow, Kind::Flowchart).unwrap().mixed, 0, "{text}");
            let d = choose(&flow, Kind::Flowchart).unwrap();
            for p in &d.paths {
                let mut at = end_point(&p[0]);
                let mut diagonal = Vec::new();
                for s in &p[1..] {
                    let q = end_point(s);
                    if let Seg::Line(q) = *s {
                        let (dx, dy) = ((q.0 - at.0).abs(), (q.1 - at.1).abs());
                        if dx >= 0.5 && dy >= 0.5 {
                            diagonal.push(dx.atan2(dy).to_degrees());
                        }
                    }
                    at = q;
                }
                // A diagonal edge is one straight line, a label cut aside, at a multiple of 15
                // degrees.
                if let Some(&a) = diagonal.first() {
                    assert!(p.iter().all(|s| !matches!(s, Seg::Curve(..))), "{p:?}");
                    for b in &diagonal {
                        assert!((a - b).abs() < 0.5, "{p:?}");
                        assert!((b / 15.0 - (b / 15.0).round()).abs() < 0.05, "{p:?}");
                    }
                }
            }
        }
    }

    /// The first and the last point of an edge.
    fn ends_of(p: &[Seg]) -> (P, P) {
        (end_point(&p[0]), end_point(&p[p.len() - 1]))
    }

    #[test]
    fn a_loop_between_2_nodes_runs_straight_both_ways() {
        // A decision and the node below it, joined by an edge down and an edge back up: both
        // run straight, the ports taking up the offset.
        let flow = parse(
            "flowchart TD\n P[Prompt arrives] --> L[Longest stored state<br/>the prompt extends]\n \
             L -->|found| R{Reading it cheaper<br/>than its prefill?}\n \
             L -->|none| F[Prefill the whole prompt]\n R -->|yes| S[Restore the state]\n \
             R -->|no| N[Next shorter stored state]\n N -->|found| R\n \
             N -->|none left| F\n S --> T[Prefill the rest]\n F --> K[Keep]\n T --> K\n",
        )
        .unwrap();
        let counted = measure(&flow, Kind::Flowchart).unwrap();
        assert_eq!(counted.jogs, 0);
        let d = choose(&flow, Kind::Flowchart).unwrap();
        for k in [4usize, 5] {
            let (s, e) = ends_of(&d.paths[k]);
            assert!((s.0 - e.0).abs() < 0.5, "edge {k}: {:?}", d.paths[k]);
        }
    }

    #[test]
    fn sources_of_equal_depth_stand_around_the_node_they_meet() {
        // 2 sources meet in a decision whose main path goes on below its middle, the other
        // children of the fork on both sides of it.
        let flow = parse(
            "flowchart TD\n m([The model]) --> g{Approval}\n y([You: screen, editor or page]) --> g\n \
             g -->|writes| f[File tools]\n g --> o[outside]\n g --> t[Tool servers]\n \
             f --> s[(Transcript)]\n f --> h[Hooks]\n",
        )
        .unwrap();
        let d = choose(&flow, Kind::Flowchart).unwrap();
        let centre = d.ends[2][0].0.0;
        // The main path leaves the middle of the decision, straight.
        let (s, e) = ends_of(&d.paths[2]);
        assert!(
            (s.0 - centre).abs() < 0.5 && (e.0 - centre).abs() < 0.5,
            "{:?}",
            d.paths[2]
        );
        // The 2 sources arrive on both sides of the middle, as far from it.
        let (a, b) = (
            ends_of(&d.paths[0]).1.0 - centre,
            ends_of(&d.paths[1]).1.0 - centre,
        );
        assert!(a * b < 0.0 && (a + b).abs() < 0.5, "{a} {b}");
        // The other children stand on both sides of the main path.
        let (l, r) = (d.ends[3][1].0.0 - centre, d.ends[4][1].0.0 - centre);
        assert!(l * r < 0.0, "{l} {r}");
    }

    #[test]
    fn a_lane_beside_its_target_arrives_by_the_side() {
        let flow = parse(
            "flowchart TD\n w[A write] --> c[Compare with what they reported before]\n \
             c -->|they cannot tell| k[The compiler, once per step]\n \
             c --> v[The verdict, in the result of the write]\n k --> v\n",
        )
        .unwrap();
        let d = choose(&flow, Kind::Flowchart).unwrap();
        let all = meets(&d);
        // The lane of c to v leaves a side of c and arrives on the same side of v.
        assert!(all[4].4 && all[5].4, "{:?}", d.paths[2]);
        assert_eq!(measure(&flow, Kind::Flowchart).unwrap().bends, 2);
    }

    #[test]
    fn edges_to_one_node_share_their_last_run() {
        let flow = parse("flowchart TD\n  call[\"A call that changes something\"] --> pre{\"The review model, planning or a pre-tool hook refuses?\"}\n  pre -->|no| deny{\"deny rule?\"}\n  deny -->|no| ask{\"ask rule?\"}\n  ask -->|no| allow{\"allow rule, under max-auto?\"}\n  allow -->|no| agreed{\"Writing without asking, or an agreement?\"}\n  agreed -->|no| you[\"You answer\"]\n  pre --> back[\"Refused, back to the model with why\"]\n  deny --> back\n  ask --> you\n  allow --> run[\"It runs\"]\n  agreed --> run\n  you -->|\"y or a\"| run\n  you -->|\"n, e or x\"| back\n").unwrap();
        let d = choose(&flow, Kind::Flowchart).unwrap();
        // pre and deny to back, allow and agreed to run.
        for (a, b) in [(6usize, 7usize), (9, 10)] {
            let (p, q) = (ends_of(&d.paths[a]).1, ends_of(&d.paths[b]).1);
            assert!(
                (p.0 - q.0).abs() < 0.5 && (p.1 - q.1).abs() < 0.5,
                "{p:?} {q:?}"
            );
        }
        let counted = measure(&flow, Kind::Flowchart).unwrap();
        assert_eq!(counted.crossings, 0);
        assert!(counted.merged >= 4);
    }
}

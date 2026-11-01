//! Flowcharts: layered layout, edge routing and drawing.

use super::Drawing;
use super::canvas::{Canvas, Dir, E, N, S, W, Weight, tint};
use super::graph::{Direction, Edge, Flow, Head, Node, Role, Shape};
use super::route::{Blocked, Search, route};
use super::vector::layered::{self, EdgeIn, Input, NodeIn, Outline, Plan, Spacing, TitleSide};
use crate::options::RenderOptions;
use crate::wrap::{display_width, segments_width};

/// Blank cells kept around the layout so that edges can route around the outer nodes.
const MARGIN: usize = 2;
/// Border plus one blank cell on each side of a label.
const NODE_PAD: usize = 4;
/// Blank cells between two layers, along the flow.
const GAP_ALONG_VERTICAL: usize = 3;
const GAP_ALONG_HORIZONTAL: usize = 5;
/// Cells added to the width of an edge label to get the gap that holds it.
const LABEL_ROOM: usize = 4;
/// Rows added to a vertical gap that holds an edge label.
const LABEL_ROWS: usize = 2;
/// Blank cells between two nodes of a layer.
const GAP_ACROSS_VERTICAL: usize = 4;
const GAP_ACROSS_HORIZONTAL: usize = 2;
/// Left and right points of a diamond node, on the row of its port.
const DIAMOND_SIDES: [char; 2] = ['<', '>'];
/// Cells between the left edge of a diamond and the first cell of its top and bottom edges.
const DIAMOND_INSET: usize = 3;
/// Rows of a diamond above and below its text.
const DIAMOND_SLANT: usize = 2;
/// Cells between the side vertex of a fork and the column where its edge turns.
const TURN: f64 = 2.0;
/// Cells kept beside an edge that runs past a layer.
const LANE: f64 = 2.0;
/// Passes of neighbour-average ordering over the layers, in the reference layout.
const ORDER_PASSES: usize = 2;
/// The colour of a node of `role` in the diagram palette of the theme.
fn node_tint(role: Role) -> u8 {
    match role {
        Role::Step => tint::STEP,
        Role::End => tint::END,
        Role::Decision => tint::DECISION,
        Role::Store => tint::STORE,
    }
}

struct Placed {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    diamond: bool,
}

impl Placed {
    /// Columns of the top and bottom border where an edge may attach.
    fn columns(&self) -> (usize, usize) {
        let inset = if self.diamond { DIAMOND_INSET } else { 1 };
        (self.x + inset, (self.x + self.w).saturating_sub(inset + 1))
    }

    /// Rows of the left and right border where an edge may attach.
    fn rows(&self) -> (usize, usize) {
        if self.diamond {
            (self.y + self.h / 2, self.y + self.h / 2)
        } else {
            (self.y + 1, (self.y + self.h).saturating_sub(2))
        }
    }

    /// Cell of the border on `side` at column or row `at`.
    fn port_at(&self, side: Dir, at: usize) -> (usize, usize) {
        match side {
            Dir::N => (at, self.y),
            Dir::S => (at, self.y + self.h - 1),
            Dir::E => (self.x + self.w - 1, at),
            Dir::W => (self.x, at),
        }
    }

    /// Cell of the border on `side` where an edge attaches. With `other`, the middle of the span the
    /// two boxes share along that border when there is one, so that the edge runs straight;
    /// without, the middle of the border.
    fn port(&self, side: Dir, other: Option<&Placed>) -> (usize, usize) {
        let span = |own: (usize, usize), theirs: (usize, usize), middle: usize| {
            let (lo, hi) = (own.0.max(theirs.0), own.1.min(theirs.1));
            if lo <= hi { (lo + hi) / 2 } else { middle }
        };
        let col = other.map_or(self.x + self.w / 2, |o| {
            span(self.columns(), o.columns(), self.x + self.w / 2)
        });
        let row = other.map_or(self.y + self.h / 2, |o| {
            span(self.rows(), o.rows(), self.y + self.h / 2)
        });
        let row = if self.diamond { self.rows().0 } else { row };
        match side {
            Dir::N => (col, self.y),
            Dir::S => (col, self.y + self.h - 1),
            Dir::E => (self.x + self.w - 1, row),
            Dir::W => (self.x, row),
        }
    }
}

/// The cells where an edge leaves `a` on side `out` and reaches `b` on side `into`. A lone end
/// takes the middle of its side, an end shared with other edges its slot.
fn ports(
    a: &Placed,
    out: Dir,
    a_at: Option<usize>,
    b: &Placed,
    into: Dir,
    b_at: Option<usize>,
) -> ((usize, usize), (usize, usize)) {
    let vertical = matches!(out, Dir::N | Dir::S);
    if vertical != matches!(into, Dir::N | Dir::S) {
        // A side exit toward a side along the flow: each end on its own middle or slot.
        let middle = |p: &Placed, side: Dir| {
            let cell = p.port(side, None);
            if matches!(side, Dir::N | Dir::S) {
                cell.0
            } else {
                cell.1
            }
        };
        let from = a_at.unwrap_or_else(|| middle(a, out));
        let to = b_at.unwrap_or_else(|| middle(b, into));
        return (a.port_at(out, from), b.port_at(into, to));
    }
    let range = |p: &Placed| if vertical { p.columns() } else { p.rows() };
    let middle = |p: &Placed| p.port(if vertical { Dir::N } else { Dir::W }, None);
    let coord = |cell: (usize, usize)| if vertical { cell.0 } else { cell.1 };
    let within = |p: &Placed, c: usize| {
        let (lo, hi) = range(p);
        lo <= c && c <= hi
    };
    // An end shared with other edges has its own column or row, given in `a_at` or `b_at`.
    let fixed = |p: &Placed, at: Option<usize>| at.unwrap_or_else(|| coord(middle(p)));
    // The 2 ends share a column or a row when it lies within a cell of the middle of each lone
    // end: the edge runs straight and each port stays at the middle of its side, rounded.
    let (ma, mb) = (coord(middle(a)), coord(middle(b)));
    let near = |c: usize, m: usize| c.abs_diff(m) <= 1;
    let shared = match (a_at.is_none(), b_at.is_none()) {
        (true, true) => {
            let width = |p: &Placed| range(p).1.saturating_sub(range(p).0);
            let order = if width(a) <= width(b) {
                [ma, mb]
            } else {
                [mb, ma]
            };
            order
                .into_iter()
                .find(|&c| within(a, c) && within(b, c) && near(c, ma) && near(c, mb))
        }
        (false, true) => Some(fixed(a, a_at)).filter(|&c| within(b, c) && near(c, mb)),
        (true, false) => Some(fixed(b, b_at)).filter(|&c| within(a, c) && near(c, ma)),
        (false, false) => None,
    };
    let end = |p: &Placed, side: Dir, at: Option<usize>, _other: &Placed| match at {
        Some(c) => p.port_at(side, c),
        None => p.port(side, None),
    };
    match shared {
        Some(c) => (a.port_at(out, c), b.port_at(into, c)),
        None => (end(a, out, a_at, b), end(b, into, b_at, a)),
    }
}

/// A route tried for an edge: its cost, the lines it runs over before its turns, the side it
/// arrives by, its 2 ports and its cells.
type Tried = (
    usize,
    Dir,
    (usize, usize),
    (usize, usize),
    Vec<(usize, usize)>,
);

/// A label that waits for every route: the path of its edge, the cells of the path shared with
/// other edges, and the text.
type Waiting = (Vec<(usize, usize)>, Vec<bool>, String);

/// An edge end on a side of a node: where the other end lies, the edge, and whether it leaves.
type End = (usize, usize, bool);

/// Columns or rows of `k` edges that share a side of a node, spread evenly. `None` when 2 of them
/// would share a cell.
///
/// The cells sit as far from the middle of the side on both sides, at least 2 apart when the
/// side has room, else at (k + 1) / (n + 1) of the side, rounded to a cell.
fn slots(p: &Placed, side: Dir, k: usize) -> Option<Vec<usize>> {
    let vertical = matches!(side, Dir::N | Dir::S);
    let (lo, hi) = if vertical { p.columns() } else { p.rows() };
    let cell = p.port(side, None);
    let middle = (if vertical { cell.0 } else { cell.1 }) as f64;
    let step = ((hi - lo) as f64 / (k + 1) as f64).max(2.0);
    let around: Option<Vec<usize>> = (0..k)
        .map(|i| {
            let off = (i as f64 - (k - 1) as f64 / 2.0) * step;
            let c = middle + off.signum() * off.abs().round();
            (c >= lo as f64 && c <= hi as f64).then_some(c as usize)
        })
        .collect();
    if let Some(cells) = around.filter(|c| c.windows(2).all(|w| w[0] < w[1])) {
        return Some(cells);
    }
    let cells: Vec<usize> = (0..k)
        .map(|i| lo + ((hi - lo) as f64 * (i + 1) as f64 / (k + 1) as f64).round() as usize)
        .collect();
    cells.windows(2).all(|w| w[0] < w[1]).then_some(cells)
}

/// Edges closing a cycle in a depth-first walk from the nodes in order. `None` for a self-loop.
fn back_edges(flow: &Flow) -> Option<Vec<bool>> {
    let n = flow.nodes.len();
    let mut out: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, e) in flow.edges.iter().enumerate() {
        if e.from == e.to {
            return None;
        }
        out[e.from].push(i);
    }
    let mut color = vec![0u8; n];
    let mut back = vec![false; flow.edges.len()];
    for root in 0..n {
        if color[root] != 0 {
            continue;
        }
        color[root] = 1;
        let mut stack = vec![(root, 0usize)];
        while let Some(top) = stack.last_mut() {
            let (u, i) = *top;
            if i == out[u].len() {
                color[u] = 2;
                stack.pop();
                continue;
            }
            top.1 += 1;
            let e = out[u][i];
            let v = flow.edges[e].to;
            match color[v] {
                0 => {
                    color[v] = 1;
                    stack.push((v, 0));
                }
                1 => back[e] = true,
                _ => {}
            }
        }
    }
    Some(back)
}

/// Rank of each node on the longest path from the sources, without the edges that close a cycle.
fn ranks(flow: &Flow, back: &[bool]) -> Vec<usize> {
    let mut rank = vec![0usize; flow.nodes.len()];
    loop {
        let mut changed = false;
        for (e, edge) in flow.edges.iter().enumerate() {
            if !back[e] && rank[edge.to] <= rank[edge.from] {
                rank[edge.to] = rank[edge.from] + 1;
                changed = true;
            }
        }
        if !changed {
            return rank;
        }
    }
}

/// Orders each layer by the average position of the neighbours in `nbrs`.
fn reorder(layer: &mut [usize], pos: &mut [usize], nbrs: &[Vec<usize>]) {
    let key = |v: usize| {
        if nbrs[v].is_empty() {
            pos[v] as f64
        } else {
            nbrs[v].iter().map(|&u| pos[u] as f64).sum::<f64>() / nbrs[v].len() as f64
        }
    };
    let mut keyed: Vec<(f64, usize)> = layer.iter().map(|&v| (key(v), v)).collect();
    keyed.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (i, (_, v)) in keyed.into_iter().enumerate() {
        layer[i] = v;
        pos[v] = i;
    }
}

/// The nodes of each rank, ordered by passes of neighbour averages.
fn swept_layers(flow: &Flow, back: &[bool], rank: &[usize]) -> Vec<Vec<usize>> {
    let depth = rank.iter().max().map_or(0, |m| m + 1);
    let mut layers: Vec<Vec<usize>> = vec![Vec::new(); depth];
    let mut pos = vec![0usize; flow.nodes.len()];
    for (v, &r) in rank.iter().enumerate() {
        pos[v] = layers[r].len();
        layers[r].push(v);
    }
    let mut preds: Vec<Vec<usize>> = vec![Vec::new(); flow.nodes.len()];
    let mut succs: Vec<Vec<usize>> = vec![Vec::new(); flow.nodes.len()];
    for (e, edge) in flow.edges.iter().enumerate() {
        if !back[e] {
            preds[edge.to].push(edge.from);
            succs[edge.from].push(edge.to);
        }
    }
    for _ in 0..ORDER_PASSES {
        for layer in layers.iter_mut().skip(1) {
            reorder(layer, &mut pos, &preds);
        }
        for layer in layers.iter_mut().rev().skip(1) {
            reorder(layer, &mut pos, &succs);
        }
    }
    layers
}

/// Size of each node across and along the grid: width and height.
fn sizes(flow: &Flow, texts: &[Vec<String>]) -> Vec<(usize, usize)> {
    flow.nodes
        .iter()
        .zip(texts)
        .map(|(n, lines)| {
            let text = lines.iter().map(|l| display_width(l)).max().unwrap_or(0);
            if n.shape == Shape::Diamond {
                (text + 2 * DIAMOND_INSET, lines.len() + 2 * DIAMOND_SLANT)
            } else {
                (text + NODE_PAD, lines.len() + 2 + n.dividers.len())
            }
        })
        .collect()
}

/// The ranks, the order and the positions across that the layered layout gives the nodes, in
/// cells, so that the grid drawing crosses as few edges as the vector one and keeps a chain
/// straight.
fn plan(flow: &Flow, sizes: &[(usize, usize)]) -> Option<Plan> {
    let horizontal = flow.direction.horizontal();
    let gap_across = if horizontal {
        GAP_ACROSS_HORIZONTAL
    } else {
        GAP_ACROSS_VERTICAL
    };
    let nodes = flow
        .nodes
        .iter()
        .zip(sizes)
        .map(|(n, &(w, h))| {
            let (across, along) = if horizontal { (h, w) } else { (w, h) };
            let half = across as f64 / 2.0;
            NodeIn {
                lo: half,
                hi: half,
                along: along as f64,
                outline: if n.shape == Shape::Diamond {
                    Outline::Diamond
                } else {
                    Outline::Rect
                },
                boxy: false,
            }
        })
        .collect();
    let edges = flow
        .edges
        .iter()
        .map(|e| EdgeIn {
            from: e.from,
            to: e.to,
            // A label is written beside its edge, on one row.
            label: e.text().map(|t| {
                let w = (display_width(&t) + 2) as f64;
                if horizontal { (1.0, w) } else { (w, 1.0) }
            }),
            heads: (0.0, 0.0),
            look: e.look(&flow.edges),
        })
        .collect();
    let input = Input {
        under_parents: false,
        nodes,
        edges,
        groups: Vec::new(),
        title_side: TitleSide::AlongLow,
        spacing: Spacing {
            node: gap_across as f64,
            rank: 1.0,
            edge: LANE,
            pad: 1.0,
            outside: 1.0,
            turn: TURN,
        },
    };
    layered::plan(&input)
}

/// The nodes of each layer, in the order of their positions across.
fn layers(rank: &[usize], across: &[f64]) -> Vec<Vec<usize>> {
    let depth = rank.iter().max().map_or(0, |m| m + 1);
    let mut layers: Vec<Vec<usize>> = vec![Vec::new(); depth];
    for (v, &r) in rank.iter().enumerate() {
        layers[r].push(v);
    }
    for layer in &mut layers {
        layer.sort_by(|&a, &b| across[a].total_cmp(&across[b]).then(a.cmp(&b)));
    }
    layers
}

/// Blank cells after each layer. A gap holds the label of an edge to the next layer: horizontal
/// flows widen it by the label width, vertical flows by `LABEL_ROWS` rows.
fn gaps(flow: &Flow, back: &[bool], rank: &[usize], depth: usize) -> Vec<usize> {
    let horizontal = flow.direction.horizontal();
    let base = if horizontal {
        GAP_ALONG_HORIZONTAL
    } else {
        GAP_ALONG_VERTICAL
    };
    let mut gaps = vec![base; depth.saturating_sub(1)];
    for (e, edge) in flow.edges.iter().enumerate() {
        if let (Some(label), false) = (&edge.text(), back[e]) {
            let r = rank[edge.from];
            gaps[r] = gaps[r].max(if horizontal {
                display_width(label) + LABEL_ROOM
            } else {
                base + LABEL_ROWS
            });
        }
    }
    gaps
}

/// Writes `label` beside a straight run of `path`, the longest first, away from the cells that
/// `joined` marks as shared with other edges while a run without them has room.
fn place_label(canvas: &mut Canvas, path: &[(usize, usize)], joined: &[bool], label: &str) -> bool {
    let width = display_width(label);
    // The straight runs of the path, as ranges of its cells, and whether they hold shared cells.
    let mut spans: Vec<(std::ops::RangeInclusive<usize>, bool)> = Vec::new();
    let mut start = 0;
    for i in 1..path.len() {
        let turns = i + 1 < path.len()
            && Dir::between(path[i - 1], path[i]) != Dir::between(path[i], path[i + 1]);
        if turns || i + 1 == path.len() {
            let shared = (start..=i).any(|k| joined.get(k).copied().unwrap_or(false));
            spans.push((start..=i, shared));
            start = i;
        }
    }
    spans.sort_by_key(|(r, shared)| (*shared, std::cmp::Reverse(r.end() - r.start())));
    let runs: Vec<&[(usize, usize)]> = spans.iter().map(|(r, _)| &path[r.clone()]).collect();
    // First with a blank cell on each side of the label, then without. Along a run, the
    // positions closest to its middle come first.
    for margin in [1, 0] {
        for run in &runs {
            let (first, last) = (run[0], run[run.len() - 1]);
            let mut candidates: Vec<(usize, usize, usize)> = Vec::new();
            if first.1 == last.1 {
                let (lo, hi) = (first.0.min(last.0), first.0.max(last.0));
                let middle = (lo + hi) / 2;
                for x in lo..=hi {
                    let distance = (x + width / 2).abs_diff(middle);
                    candidates.push((distance, x, first.1.wrapping_sub(1)));
                    candidates.push((distance, x, first.1 + 1));
                }
            } else {
                let (lo, hi) = (first.1.min(last.1), first.1.max(last.1));
                let middle = (lo + hi) / 2;
                for y in lo..=hi {
                    let distance = y.abs_diff(middle);
                    candidates.push((distance, first.0 + 1 + margin, y));
                    candidates.push((distance, first.0.wrapping_sub(width + margin), y));
                }
            }
            candidates.sort_by_key(|&(distance, ..)| distance);
            for (_, x, y) in candidates {
                let fits = x >= margin
                    && x.checked_add(width + margin)
                        .is_some_and(|end| end < canvas.width())
                    && y < canvas.height()
                    && (x - margin..x + width + margin).all(|cx| canvas.is_empty(cx, y));
                if fits {
                    canvas.text(x, y, label);
                    let cells: Vec<(usize, usize)> = (x..x + width).map(|cx| (cx, y)).collect();
                    canvas.tint(&cells, tint::LABEL);
                    return true;
                }
            }
        }
    }
    false
}

/// Places the nodes: each layer in its band along the flow, each node across at the position of
/// the plan, rounded to a cell so that nodes that face each other share their middle column.
fn place(
    flow: &Flow,
    sizes: &[(usize, usize)],
    layers: &[Vec<usize>],
    gaps: &[usize],
    across_at: Option<&[f64]>,
) -> (Vec<Placed>, usize, usize) {
    let horizontal = flow.direction.horizontal();
    let gap_across = if horizontal {
        GAP_ACROSS_HORIZONTAL
    } else {
        GAP_ACROSS_VERTICAL
    };
    let along = |v: usize| if horizontal { sizes[v].0 } else { sizes[v].1 };
    let across = |v: usize| if horizontal { sizes[v].1 } else { sizes[v].0 };
    let spans: Vec<usize> = layers
        .iter()
        .map(|l| l.iter().map(|&v| along(v)).max().unwrap_or(0))
        .collect();
    // Start of each node across: its middle cell at the rounded position, then pushed right
    // where it would touch the node before it. Without positions, each layer is packed and
    // centred.
    let mut start = vec![0i64; flow.nodes.len()];
    let widths: Vec<usize> = layers
        .iter()
        .map(|l| {
            l.iter().map(|&v| across(v)).sum::<usize>() + gap_across * l.len().saturating_sub(1)
        })
        .collect();
    let widest = widths.iter().copied().max().unwrap_or(0);
    let low = across_at.map_or(0.0, |a| a.iter().copied().fold(f64::INFINITY, f64::min));
    for (r, layer) in layers.iter().enumerate() {
        let mut free = i64::try_from((widest - widths[r]) / 2).unwrap_or(0);
        if across_at.is_some() {
            free = i64::MIN;
        }
        for &v in layer {
            let s = match across_at {
                Some(at) => {
                    let middle = (at[v] - low).round() as i64;
                    (middle - i64::try_from(across(v) / 2).unwrap_or(0)).max(free)
                }
                None => free,
            };
            start[v] = s;
            free = s + i64::try_from(across(v) + gap_across).unwrap_or(0);
        }
    }
    let first = start.iter().copied().min().unwrap_or(0);
    let cross: Vec<usize> = start
        .iter()
        .map(|&s| usize::try_from(s - first).unwrap_or(0))
        .collect();
    let total_across = (0..flow.nodes.len())
        .map(|v| cross[v] + across(v))
        .max()
        .unwrap_or(0);
    let total_along = spans.iter().sum::<usize>() + gaps.iter().sum::<usize>();
    let mirrored = matches!(flow.direction, Direction::BottomUp | Direction::RightLeft);
    let mut placed: Vec<Placed> = sizes
        .iter()
        .zip(&flow.nodes)
        .map(|(&(w, h), n)| Placed {
            x: 0,
            y: 0,
            w,
            h,
            diamond: n.shape == Shape::Diamond,
        })
        .collect();
    let mut offset = 0;
    for (r, layer) in layers.iter().enumerate() {
        for &v in layer {
            // A box takes the length of the longest of its layer along the flow; a diamond
            // keeps its own.
            let boxes = layer
                .iter()
                .filter(|&&u| !placed[u].diamond)
                .map(|&u| along(u))
                .max()
                .unwrap_or(0);
            let length = if placed[v].diamond { along(v) } else { boxes };
            if horizontal {
                placed[v].w = length;
            } else {
                placed[v].h = length;
            }
            // Every node of a layer centred on one row, so that a side branch runs straight.
            let mut main = offset + spans[r] / 2 - length / 2;
            if mirrored {
                main = total_along - main - length;
            }
            let (x, y) = if horizontal {
                (main, cross[v])
            } else {
                (cross[v], main)
            };
            placed[v].x = x + MARGIN;
            placed[v].y = y + MARGIN;
        }
        offset += spans[r] + gaps.get(r).copied().unwrap_or(0);
    }
    let (w, h) = if horizontal {
        (total_along, total_across)
    } else {
        (total_across, total_along)
    };
    (placed, w, h)
}

/// Text of a node: its lines, or the glyph of a start or end state.
fn node_text(node: &Node, opts: &RenderOptions) -> Vec<String> {
    match node.shape {
        Shape::Start => vec![opts.glyphs.start_state.clone()],
        Shape::End => vec![opts.glyphs.end_state.clone()],
        _ => node.lines.clone(),
    }
}

/// Draws a node: its border, whose corners depend on the shape, and its text.
fn draw_node(canvas: &mut Canvas, node: &Node, text: &[String], p: &Placed, opts: &RenderOptions) {
    draw_shape(canvas, node, text, p, opts);
    canvas.tint_lines(p.x, p.y, p.w, p.h, node_tint(node.shape.role()));
}

/// Draws the outline of a node and its text.
fn draw_shape(canvas: &mut Canvas, node: &Node, text: &[String], p: &Placed, opts: &RenderOptions) {
    if node.shape == Shape::Diamond {
        return draw_diamond(canvas, text, p, opts);
    }
    canvas.draw_box(p.x, p.y, p.w, p.h);
    if matches!(
        node.shape,
        Shape::Round
            | Shape::Stadium
            | Shape::Circle
            | Shape::DoubleCircle
            | Shape::Start
            | Shape::End
    ) {
        let [tl, tr, bl, br] = opts.glyphs.rounded;
        canvas.set_glyph(p.x, p.y, tl);
        canvas.set_glyph(p.x + p.w - 1, p.y, tr);
        canvas.set_glyph(p.x, p.y + p.h - 1, bl);
        canvas.set_glyph(p.x + p.w - 1, p.y + p.h - 1, br);
    }
    let first_divider = node.dividers.first().copied().unwrap_or(usize::MAX);
    // The text sits in the middle of a box taller than it.
    let natural = text.len() + node.dividers.len() + 2;
    let mut y = p.y + 1 + p.h.saturating_sub(natural) / 2;
    for (i, line) in text.iter().enumerate() {
        if node.dividers.contains(&i) {
            canvas.add_lines(p.x, y, N | S | E, Weight::Normal);
            canvas.add_lines(p.x + p.w - 1, y, N | S | W, Weight::Normal);
            for x in p.x + 1..p.x + p.w - 1 {
                canvas.add_lines(x, y, E | W, Weight::Normal);
            }
            y += 1;
        }
        let pad = if i < first_divider {
            (p.w - 2 - display_width(line)) / 2
        } else {
            1
        };
        canvas.text(p.x + 1 + pad, y, line);
        y += 1;
    }
}

/// Draws a diamond: two slanted rows above and below the label, pointed on its row.
fn draw_diamond(canvas: &mut Canvas, text: &[String], p: &Placed, opts: &RenderOptions) {
    let [tl, tr, bl, br] = opts.glyphs.diamond;
    let (left, right) = (p.x, p.x + p.w - 1);
    let (top, bottom) = (p.y, p.y + p.h - 1);
    for x in left + DIAMOND_INSET..right + 1 - DIAMOND_INSET {
        canvas.add_lines(x, top, E | W, Weight::Normal);
        canvas.add_lines(x, bottom, E | W, Weight::Normal);
    }
    canvas.set_glyph(left + 2, top, tl);
    canvas.set_glyph(right - 2, top, tr);
    canvas.set_glyph(left + 2, bottom, bl);
    canvas.set_glyph(right - 2, bottom, br);
    canvas.set_glyph(left + 1, top + 1, tl);
    canvas.set_glyph(right - 1, top + 1, tr);
    canvas.set_glyph(left + 1, bottom - 1, bl);
    canvas.set_glyph(right - 1, bottom - 1, br);
    let tip = p.rows().0;
    for (i, line) in text.iter().enumerate() {
        let y = top + DIAMOND_SLANT + i;
        if y == tip {
            canvas.set_glyph(left, y, DIAMOND_SIDES[0]);
            canvas.set_glyph(right, y, DIAMOND_SIDES[1]);
        } else {
            canvas.add_lines(left, y, N | S, Weight::Normal);
            canvas.add_lines(right, y, N | S, Weight::Normal);
        }
        let pad = (p.w - 2 * DIAMOND_INSET - display_width(line)) / 2;
        canvas.text(left + DIAMOND_INSET + pad, y, line);
    }
}

/// The glyph of an edge end that points along `heading`, if the end has a head.
fn head_glyph(head: Head, heading: Dir, opts: &RenderOptions) -> Option<char> {
    match head {
        Head::None | Head::One | Head::ZeroOne | Head::OneMany | Head::ZeroMany => None,
        Head::Arrow | Head::Circle | Head::Cross => Some(opts.glyphs.arrows[heading.index()]),
        Head::Inherit => Some(opts.glyphs.inherit[heading.index()]),
        Head::Composition => Some(opts.glyphs.composition),
        Head::Aggregation => Some(opts.glyphs.aggregation),
    }
}

/// Draws the flowchart within `width` columns. `None` when it does not fit or cannot be routed.
/// A drawing of the grid with what decides between layouts: crossings, bends and size.
struct Candidate {
    drawing: Drawing,
    crossings: usize,
    bends: usize,
    w: usize,
    h: usize,
}

pub fn draw(flow: &Flow, width: usize, opts: &RenderOptions) -> Option<Drawing> {
    if flow.nodes.is_empty() {
        return None;
    }
    let old_back = back_edges(flow)?;
    let texts: Vec<Vec<String>> = flow.nodes.iter().map(|n| node_text(n, opts)).collect();
    let sizes = sizes(flow, &texts);
    // The reference: ranks and passes of the first layout, each layer packed. Then the order and
    // the positions of the layered layout, and its order packed. The layout with the fewest
    // crossings wins, then the fewest bends, then the smallest.
    let old_rank = ranks(flow, &old_back);
    let old_layers = swept_layers(flow, &old_back, &old_rank);
    let planned = plan(flow, &sizes);
    let main = planned.as_ref().map(|p| p.main.clone()).unwrap_or_default();
    let reference = render(
        flow,
        width,
        opts,
        &texts,
        &sizes,
        &old_back,
        &old_rank,
        &old_layers,
        None,
        false,
        &main,
        None,
    );
    let mut best = reference;
    // A try counts once it beats the best so far, which bounds the next ones.
    let offer = |c: Option<Candidate>, best: &mut Option<Candidate>| {
        if let Some(c) = c {
            let take = match best {
                None => true,
                // Fewer crossings, then fewer bends, then a smaller area; a larger area is fine.
                Some(b) => (c.crossings, c.bends, c.w * c.h) < (b.crossings, b.bends, b.w * b.h),
            };
            if take {
                *best = Some(c);
            }
        }
    };
    let bound = |best: &Option<Candidate>| best.as_ref().map(|b| (b.crossings, b.bends));
    let split = render(
        flow,
        width,
        opts,
        &texts,
        &sizes,
        &old_back,
        &old_rank,
        &old_layers,
        None,
        true,
        &main,
        bound(&best),
    );
    offer(split, &mut best);
    if let Some(plan) = planned {
        let layers = layers(&plan.rank, &plan.across);
        for at in [Some(plan.across.as_slice()), None] {
            for split in [false, true] {
                let c = render(
                    flow,
                    width,
                    opts,
                    &texts,
                    &sizes,
                    &plan.back,
                    &plan.rank,
                    &layers,
                    at,
                    split,
                    &main,
                    bound(&best),
                );
                offer(c, &mut best);
            }
        }
    }
    best.map(|c| c.drawing)
}

/// Draws `flow` with the nodes of each rank in `layers`, at the positions across `at` or packed.
/// With `split`, the edges that leave one side of a node leave from cells of their own. `None`
/// when it fails, or when its crossings, then its turns, pass `bound`.
#[allow(clippy::too_many_arguments)]
fn render(
    flow: &Flow,
    width: usize,
    opts: &RenderOptions,
    texts: &[Vec<String>],
    sizes: &[(usize, usize)],
    back: &[bool],
    rank: &[usize],
    layers: &[Vec<usize>],
    at: Option<&[f64]>,
    split: bool,
    main: &[bool],
    bound: Option<(usize, usize)>,
) -> Option<Candidate> {
    let gaps = gaps(flow, back, rank, layers.len());
    let (placed, w, h) = place(flow, sizes, layers, &gaps, at);
    // The nodes alone already span more than the width.
    let left = placed.iter().map(|p| p.x).min().unwrap_or(0);
    let right = placed.iter().map(|p| p.x + p.w).max().unwrap_or(0);
    if right - left > width {
        return None;
    }
    // In a vertical flow a label is written beside its edge and may run past the layout.
    let room = if flow.direction.horizontal() {
        0
    } else {
        flow.edges
            .iter()
            .filter_map(Edge::text)
            .map(|label| display_width(&label) + 3)
            .max()
            .unwrap_or(0)
    };
    let (cw, ch) = (w + 2 * MARGIN + room, h + 2 * MARGIN);
    let mut canvas = Canvas::new(cw, ch);
    let mut blocked = Blocked::new(cw, ch);
    for ((node, text), p) in flow.nodes.iter().zip(texts).zip(&placed) {
        draw_node(&mut canvas, node, text, p, opts);
        blocked.block_rect(p.x, p.y, p.w, p.h);
    }
    let flow_dir = flow.direction.flow();
    let side = flow.direction.side();
    let horizontal = flow.direction.horizontal();
    // A fork of 2 or 3 sends a child that lies past one of its sides out of that side: its
    // vertex for a diamond, the middle of the side for a box.
    let mut forward_out = vec![0usize; flow.nodes.len()];
    for (e, edge) in flow.edges.iter().enumerate() {
        if !back[e] {
            forward_out[edge.from] += 1;
        }
    }
    let sides: Vec<(Dir, Dir)> = flow
        .edges
        .iter()
        .enumerate()
        .map(|(e, edge)| {
            if back[e] {
                return (side, side);
            }
            let (a, b) = (&placed[edge.from], &placed[edge.to]);
            if rank[edge.from] == rank[edge.to] {
                // A side branch in the layer of its parent: across, from the facing sides.
                let out = match (horizontal, b.x > a.x, b.y > a.y) {
                    (false, true, _) => Dir::E,
                    (false, false, _) => Dir::W,
                    (true, _, true) => Dir::S,
                    (true, _, false) => Dir::N,
                };
                return (out, out.opposite());
            }
            let (lo, hi, aim) = if horizontal {
                (a.y, a.y + a.h - 1, b.y + b.h / 2)
            } else {
                (a.x, a.x + a.w - 1, b.x + b.w / 2)
            };
            let out = if !(2..=3).contains(&forward_out[edge.from]) {
                flow_dir
            } else if aim < lo {
                if horizontal { Dir::N } else { Dir::W }
            } else if aim > hi {
                if horizontal { Dir::S } else { Dir::E }
            } else {
                flow_dir
            };
            (out, flow_dir.opposite())
        })
        .collect();
    // The ends that share a side, in the order of the other ends.
    let mut sharing: std::collections::BTreeMap<(usize, usize), Vec<End>> =
        std::collections::BTreeMap::new();
    for (e, (edge, &(out, in_side))) in flow.edges.iter().zip(&sides).enumerate() {
        let toward = |p: &Placed, side: Dir| {
            if matches!(side, Dir::N | Dir::S) {
                p.x + p.w / 2
            } else {
                p.y + p.h / 2
            }
        };
        sharing.entry((edge.from, out.index())).or_default().push((
            toward(&placed[edge.to], out),
            e,
            true,
        ));
        sharing
            .entry((edge.to, in_side.index()))
            .or_default()
            .push((toward(&placed[edge.from], in_side), e, false));
    }
    let mut at: Vec<(Option<usize>, Option<usize>)> = vec![(None, None); flow.edges.len()];
    for list in sharing.values_mut() {
        list.sort();
    }
    // Edges that leave a side each take their cell. Edges that arrive meet in one cell: with
    // split ends, the cell of an edge that comes straight from the node it leaves, else the
    // middle.
    for arriving in [false, true] {
        for (&(node, side), list) in &sharing {
            let all_arrive = list.iter().all(|l| !l.2);
            if list.len() < 2 || all_arrive != arriving {
                continue;
            }
            let side = Dir::ALL[side];
            let vertical = matches!(side, Dir::N | Dir::S);
            let cell = placed[node].port(side, None);
            let middle = if vertical { cell.0 } else { cell.1 };
            let (lo, hi) = if vertical {
                placed[node].columns()
            } else {
                placed[node].rows()
            };
            let cells = slots(&placed[node], side, list.len());
            let meet = list
                .iter()
                .filter(|&&(_, e, _)| rank[flow.edges[e].to] == rank[flow.edges[e].from] + 1)
                .filter_map(|&(_, e, _)| at[e].0)
                .filter(|&c| split && lo <= c && c <= hi)
                .min_by_key(|&c| (c.abs_diff(middle), c))
                .unwrap_or(middle);
            let leaving = split && list.iter().all(|l| l.2);
            for (i, &(_, e, from)) in list.iter().enumerate() {
                let c = match &cells {
                    Some(c) if leaving => c[i],
                    _ if arriving => meet,
                    _ => middle,
                };
                if from {
                    at[e].0 = Some(c);
                } else {
                    at[e].1 = Some(c);
                }
            }
        }
    }
    // A cell an edge leaves from, shared with other edges on its side, moves onto the column or
    // row of where its edge arrives, so that it runs straight, when the side holds it: the
    // cells keep their order and a free cell between them, and the others give way.
    for (&(node, side_index), list) in &sharing {
        let side = Dir::ALL[side_index];
        if list.len() < 2 || !list.iter().all(|l| l.2) || side != flow_dir {
            continue;
        }
        let vertical = matches!(side, Dir::N | Dir::S);
        let (lo, hi) = if vertical {
            placed[node].columns()
        } else {
            placed[node].rows()
        };
        // Where each edge of the side arrives across, when it arrives straight ahead.
        let aim = |e: usize, at: &[(Option<usize>, Option<usize>)]| {
            let edge = &flow.edges[e];
            if back[e] || rank[edge.to] == rank[edge.from] {
                return None;
            }
            let cell = placed[edge.to].port(sides[e].1, None);
            Some(at[e].1.unwrap_or(if vertical { cell.0 } else { cell.1 }))
        };
        // The cells beyond the moved one give way, 2 apart, unless their edge runs straight.
        for i in 0..list.len() {
            let e = list[i].1;
            let (Some(to), Some(now)) = (aim(e, &at), at[e].0) else {
                continue;
            };
            if to == now || to < lo || to > hi {
                continue;
            }
            let mut moves = vec![(e, to)];
            let mut fits = true;
            let mut edge_at = to;
            for l in &list[i + 1..] {
                let cur = at[l.1].0.unwrap_or(edge_at);
                let want = cur.max(edge_at + 2);
                if want != cur && (aim(l.1, &at) == Some(cur) || want > hi) {
                    fits = false;
                    break;
                }
                moves.push((l.1, want));
                edge_at = want;
            }
            edge_at = to;
            for l in list[..i].iter().rev() {
                if !fits {
                    break;
                }
                let cur = at[l.1].0.unwrap_or(edge_at);
                let want = cur.min(edge_at.saturating_sub(2));
                if want != cur && (aim(l.1, &at) == Some(cur) || want < lo || edge_at < lo + 2) {
                    fits = false;
                    break;
                }
                moves.push((l.1, want));
                edge_at = want;
            }
            if fits {
                for (f, c) in moves {
                    at[f].0 = Some(c);
                }
            }
        }
    }
    let mut routes = Vec::new();
    let mut tally = super::Tally::default();
    let mut search = Search::default();
    let mut deferred: Vec<Waiting> = Vec::new();
    // The edges whose lines each cell holds: one class of edges toward the same node and of
    // the same look, which later edges of that class may join, or none to join.
    let mut class: Vec<Option<Option<(usize, usize)>>> = vec![None; cw * ch];
    for (e, (edge, &(out, first_side))) in flow.edges.iter().zip(&sides).enumerate() {
        // An edge toward a node joins the lines of the edges of its look toward it, but not a
        // back edge, and no edge joins one with a label.
        let key = (edge.to, edge.look(&flow.edges));
        let share: Vec<bool> = if back[e] {
            Vec::new()
        } else {
            class.iter().map(|c| *c == Some(Some(key))).collect()
        };
        // A forward edge over several ranks may also arrive by a free side across the flow of
        // its target: the route with the fewest turns wins, the side along the flow on a tie.
        let mut options = vec![first_side];
        if !back[e] && rank[edge.to] > rank[edge.from] + 1 {
            let across = if horizontal {
                [Dir::N, Dir::S]
            } else {
                [Dir::W, Dir::E]
            };
            options.extend(
                across
                    .into_iter()
                    .filter(|d| !sharing.contains_key(&(edge.to, d.index()))),
            );
        }
        let mut best: Option<Tried> = None;
        for in_side in options {
            let arrive = in_side.opposite();
            let shared_end = if in_side == first_side { at[e].1 } else { None };
            let (source, target) = ports(
                &placed[edge.from],
                out,
                at[e].0,
                &placed[edge.to],
                in_side,
                shared_end,
            );
            let Some(path) = route(
                &mut search,
                &canvas,
                &blocked,
                (out.step(source), out),
                (in_side.step(target), arrive),
                &share,
            ) else {
                continue;
            };
            // Lines of other edges the route runs over weigh more than all its turns.
            let mut heading = out;
            let mut turns = 0;
            for (i, &cell) in path.iter().enumerate() {
                let next = path.get(i + 1).map_or(arrive, |&n| Dir::between(cell, n));
                if next != heading {
                    turns += 1;
                    heading = next;
                }
                let joined = share.get(cell.1 * cw + cell.0).copied().unwrap_or(false);
                if canvas.lines(cell.0, cell.1) != 0 && !joined {
                    turns += path.len() + 4;
                }
            }
            if best.as_ref().is_none_or(|b| turns < b.0) {
                best = Some((turns, in_side, source, target, path));
            }
        }
        let (_, in_side, source, target, path) = best?;
        let arrive = in_side.opposite();
        let joined: Vec<bool> = path
            .iter()
            .map(|&(x, y)| share.get(y * cw + x).copied().unwrap_or(false))
            .collect();
        let alone = !back[e] && edge.text().is_none();
        for &(x, y) in &path {
            let c = &mut class[y * cw + x];
            *c = match *c {
                None if alone => Some(Some(key)),
                Some(Some(k)) if k == key && alone => Some(Some(k)),
                _ => Some(None),
            };
        }
        canvas.add_lines(source.0, source.1, out.bit(), Weight::Normal);
        routes.push((path.clone(), out, arrive));
        tally.add(&routes[routes.len() - 1]);
        // Crossings and turns only grow: past the bound, this layout cannot win.
        if bound.is_some_and(|b| (tally.crossings, tally.bends) > b) {
            return None;
        }
        let last = path.len() - 1;
        for (i, &cell) in path.iter().enumerate() {
            let before = if i == 0 {
                out.opposite()
            } else {
                Dir::between(cell, path[i - 1])
            };
            if i < last {
                let after = Dir::between(cell, path[i + 1]);
                canvas.add_lines(cell.0, cell.1, before.bit() | after.bit(), edge.weight);
            } else if let Some(glyph) = head_glyph(edge.to_end, arrive, opts) {
                canvas.set_glyph(cell.0, cell.1, glyph);
            } else {
                canvas.add_lines(cell.0, cell.1, before.bit() | arrive.bit(), edge.weight);
                canvas.add_lines(target.0, target.1, in_side.bit(), Weight::Normal);
            }
        }
        if let Some(glyph) = head_glyph(edge.from_end, out.opposite(), opts)
            && path.len() > 1
        {
            canvas.set_glyph(path[0].0, path[0].1, glyph);
        }
        if main.get(e).copied().unwrap_or(false) {
            canvas.tint(&path, tint::MAIN);
        }
        // With split ends, the labels wait until every edge has its route, so that no route
        // steps around a label.
        if let Some(label) = edge.text() {
            if split {
                deferred.push((path, joined, label));
            } else if !place_label(&mut canvas, &path, &joined, &label) {
                return None;
            }
        }
    }
    for (path, joined, label) in &deferred {
        if !place_label(&mut canvas, path, joined, label) {
            return None;
        }
    }
    let lines = canvas.render_lines(
        &opts.glyphs,
        opts.theme.table_border,
        opts.theme.base,
        &opts.theme.diagram,
    );
    if lines.iter().any(|l| segments_width(l) > width) {
        return None;
    }
    let (crossings, bends) = (tally.crossings, tally.bends);
    let (w, h) = (
        lines.iter().map(|l| segments_width(l)).max().unwrap_or(0),
        lines.len(),
    );
    Some(Candidate {
        drawing: Drawing {
            lines,
            canvas,
            routes,
            boxes: placed
                .iter()
                .map(|p| (p.x, p.y, p.w, p.h, p.diamond))
                .collect(),
            links: flow.edges.iter().map(|e| (e.from, e.to)).collect(),
            horizontal: flow.direction.horizontal(),
        },
        crossings,
        bends,
        w,
        h,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::parse::parse;

    fn draw_plain(src: &str, width: usize) -> Option<String> {
        let flow = parse(src)?;
        let lines = draw(&flow, width, &RenderOptions::plain(width))?.lines;
        Some(
            lines
                .iter()
                .map(|l| l.iter().map(|s| s.text.as_str()).collect::<String>() + "\n")
                .collect(),
        )
    }

    #[test]
    fn shapes_change_the_corners() {
        let art = draw_plain("graph TD\n A(x) --> B{y}\n", 40).unwrap();
        assert!(art.contains(".---."), "{art}");
    }

    #[test]
    fn diamond_has_slanted_sides_and_pointed_tips() {
        let art = draw_plain("graph TD\n B{yes}\n", 40).unwrap();
        assert_eq!(art, "  /---\\\n /     \\\n<  yes  >\n \\     /\n  \\---/\n");
    }

    #[test]
    fn edge_label_is_written_next_to_the_edge() {
        let art = draw_plain("graph LR\n A -->|ok| B\n", 40).unwrap();
        assert_eq!(
            art,
            "+---+ ok   +---+\n| A +----->| B |\n+---+      +---+\n"
        );
    }

    #[test]
    fn dotted_and_thick_edges_use_their_own_glyphs() {
        let dotted = draw_plain("graph LR\n A -.-> B\n", 40).unwrap();
        assert!(dotted.contains("....>"), "{dotted}");
        let thick = draw_plain("graph LR\n A ==> B\n", 40).unwrap();
        assert!(thick.contains("====>"), "{thick}");
    }

    #[test]
    fn label_without_room_falls_back() {
        assert!(draw_plain("graph TD\n A -->|a very long label| B\n", 12).is_none());
    }

    #[test]
    fn top_down_chain() {
        let art = draw_plain("graph TD\n A[Start] --> B[End]\n", 40).unwrap();
        assert_eq!(
            art,
            "+-------+\n| Start |\n+---+---+\n    |\n    |\n    v\n +-----+\n | End |\n +-----+\n"
        );
    }

    #[test]
    fn left_right_chain() {
        let art = draw_plain("graph LR\n A --> B\n", 40).unwrap();
        assert_eq!(art, "+---+     +---+\n| A +---->| B |\n+---+     +---+\n");
    }

    #[test]
    fn too_wide_falls_back() {
        assert!(draw_plain("graph LR\n A --> B\n", 10).is_none());
    }

    #[test]
    fn cycle_is_drawn() {
        let art = draw_plain("graph TD\n A --> B\n B --> A\n", 40).unwrap();
        assert!(art.contains('A') && art.contains('B'));
    }

    #[test]
    fn self_loop_falls_back() {
        assert!(draw_plain("graph TD\n A --> A\n", 40).is_none());
    }

    /// Turns of an edge route of a grid drawing.
    fn turns(route: &crate::mermaid::Route) -> usize {
        let (path, out, arrive) = route;
        let mut heading = *out;
        let mut count = 0;
        for (i, &cell) in path.iter().enumerate() {
            let next = path.get(i + 1).map_or(*arrive, |&n| Dir::between(cell, n));
            count += usize::from(next != heading);
            heading = next;
        }
        count
    }

    #[test]
    fn a_chain_stays_straight() {
        let flow =
            parse("graph TD\n A[Start here] --> B[Go] --> C[A longer step] --> D[End]\n").unwrap();
        let d = draw(&flow, 80, &RenderOptions::plain(80)).unwrap();
        assert!(d.routes.iter().all(|r| turns(r) == 0));
    }

    #[test]
    fn an_edge_between_nodes_of_different_widths_has_no_bend() {
        for src in [
            "graph TD\n A[Wide wide box] --> B[ab]\n",
            "graph TD\n A[ab] --> B[Wide box!]\n",
        ] {
            let flow = parse(src).unwrap();
            let d = draw(&flow, 80, &RenderOptions::plain(80)).unwrap();
            assert_eq!(turns(&d.routes[0]), 0, "{src}");
        }
    }

    #[test]
    fn the_chain_of_rules_has_no_elbow() {
        let flow = parse("flowchart TD\n  call[\"A call that changes something\"] --> pre{\"The review model, planning or a pre-tool hook refuses?\"}\n  pre -->|no| deny{\"deny rule?\"}\n  deny -->|no| ask{\"ask rule?\"}\n  ask -->|no| allow{\"allow rule, under max-auto?\"}\n  allow -->|no| agreed{\"Writing without asking, or an agreement?\"}\n  agreed -->|no| you[\"You answer\"]\n  pre --> back[\"Refused, back to the model with why\"]\n  deny --> back\n  ask --> you\n  allow --> run[\"It runs\"]\n  agreed --> run\n  you -->|\"y or a\"| run\n  you -->|\"n, e or x\"| back\n").unwrap();
        let d = draw(&flow, 200, &RenderOptions::plain(200)).unwrap();
        // pre to deny, deny to ask, ask to allow, allow to agreed.
        for e in 1..5 {
            assert_eq!(turns(&d.routes[e]), 0, "edge {e}");
        }
    }

    #[test]
    fn edges_to_one_node_share_their_last_run() {
        let src = "flowchart TD\n  call[\"A call that changes something\"] --> pre{\"The review model, planning or a pre-tool hook refuses?\"}\n  pre -->|no| deny{\"deny rule?\"}\n  deny -->|no| ask{\"ask rule?\"}\n  ask -->|no| allow{\"allow rule, under max-auto?\"}\n  allow -->|no| agreed{\"Writing without asking, or an agreement?\"}\n  agreed -->|no| you[\"You answer\"]\n  pre --> back[\"Refused, back to the model with why\"]\n  deny --> back\n  ask --> you\n  allow --> run[\"It runs\"]\n  agreed --> run\n  you -->|\"y or a\"| run\n  you -->|\"n, e or x\"| back\n";
        let grid = crate::mermaid::measure(src).grid.unwrap();
        assert_eq!(grid.crossings, 0);
        assert!(grid.merged >= 4, "{grid:?}");
    }

    #[test]
    fn edges_leaving_one_side_sit_around_its_middle() {
        let p = Placed {
            x: 0,
            y: 0,
            w: 21,
            h: 3,
            diamond: false,
        };
        let middle = p.port(Dir::S, None).0;
        let cells = slots(&p, Dir::S, 3).unwrap();
        assert_eq!(cells[1], middle);
        assert_eq!(middle - cells[0], cells[2] - middle);
    }

    #[test]
    fn a_long_edge_beside_its_target_arrives_by_the_side() {
        let flow = parse(
            "flowchart TD\n w[A write] --> ls{A language server knows the file?}\n \
             ls -->|yes| tell[The servers are told: changed and saved]\n \
             ls -->|no| cc[The compiler, once per step, bounded wait]\n \
             tell --> idle[Wait until they have no work left]\n \
             idle --> cmp[Compare with what they reported before]\n \
             cmp -->|they cannot tell| cc\n cmp --> v[The verdict, in the result]\n cc --> v\n",
        )
        .unwrap();
        let d = draw(&flow, 200, &RenderOptions::plain(200)).unwrap();
        // ls to cc arrives across the flow, with fewer turns than by the top.
        let (_, _, arrive) = d.routes[2];
        assert!(matches!(arrive, Dir::E | Dir::W), "{arrive:?}");
    }
}

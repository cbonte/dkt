//! Layered layout of a graph with nested groups, in the space of the ranks: `across` runs along a
//! rank, `along` runs from one rank to the next.
//!
//! Ranks come from the longest path, with the edges that close a cycle turned around. Every edge
//! spans 2 ranks or more: the odd ranks hold the bends of the edges and their labels, so that an
//! edge never runs through a node or a label. The order inside a rank comes from barycenter
//! sweeps that keep the members of a group together, between 2 border items. Positions across the
//! ranks come from a projection on the separation constraints, repeated toward the neighbours of
//! each item.

use std::collections::BTreeMap;

mod order;
mod place;
mod route;
mod tracks;

use tracks::{Step, square_levels, square_tracks};

/// How the outline of a node meets an edge that arrives along the flow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outline {
    Rect,
    Diamond,
    /// Half circles at the ends of the longer axis, a circle when both axes are equal.
    Round,
    /// Pointed at both ends along the flow, the points this far in at the corners.
    Hexagon(f64),
    /// Half ellipses of this height at both ends along the flow.
    Cylinder(f64),
    /// A rectangle whose sides along the flow are this much shorter at each end.
    Narrow(f64),
}

/// Distance along the flow from the centre of a node to its outline, at `dx` across from the
/// centre, for a node `across` wide and `along` long.
pub fn reach(outline: Outline, across: f64, along: f64, dx: f64) -> f64 {
    let (ha, hl) = (across / 2.0, along / 2.0);
    let dx = dx.abs().min(ha);
    match outline {
        Outline::Rect | Outline::Narrow(_) => hl,
        Outline::Diamond => hl * (1.0 - dx / ha.max(1e-9)),
        Outline::Hexagon(inset) => hl - inset * dx / ha.max(1e-9),
        Outline::Cylinder(ry) => hl - ry + ry * (1.0 - (dx / ha.max(1e-9)).powi(2)).max(0.0).sqrt(),
        Outline::Round => {
            let r = ha.min(hl);
            if ha >= hl {
                let flat = ha - r;
                if dx <= flat {
                    hl
                } else {
                    (r * r - (dx - flat).powi(2)).max(0.0).sqrt()
                }
            } else {
                hl - r + (r * r - dx * dx).max(0.0).sqrt()
            }
        }
    }
}

/// Distance across the flow from the centre of `node` to its outline, at `dy` along the flow from
/// the centre, for a node `along` long: where an edge meets a side across the flow.
fn side_across(node: &NodeIn, along: f64, dy: f64) -> f64 {
    let half = along / 2.0;
    match node.outline {
        Outline::Diamond => node.lo * (1.0 - dy.abs() / half.max(1e-9)),
        Outline::Round => reach(Outline::Round, along, 2.0 * node.lo, dy),
        _ => node.lo,
    }
}

/// How far across from its centre a port may move to keep its edge straight: half a diamond, so
/// that an edge meets it near its vertex, else the reach of the side.
fn port_room(node: &NodeIn) -> f64 {
    match node.outline {
        Outline::Diamond => node.lo * 0.5,
        _ => side_reach(node),
    }
}

/// How far across its side a node lets an edge attach, from its centre.
fn side_reach(node: &NodeIn) -> f64 {
    match node.outline {
        Outline::Diamond => node.lo,
        Outline::Round | Outline::Hexagon(_) => node.lo * 0.5,
        Outline::Rect | Outline::Cylinder(_) => (node.lo - 8.0).max(0.0),
        Outline::Narrow(inset) => (node.lo - inset - 4.0).max(0.0),
    }
}

#[derive(Clone)]
pub struct NodeIn {
    /// Extent across the flow before and after the centre. The shape takes `lo` on both sides,
    /// and `hi` may keep more room after it.
    pub lo: f64,
    pub hi: f64,
    /// Extent along the flow.
    pub along: f64,
    pub outline: Outline,
    /// A box of text, which shares its length along the flow with the boxes of its rank.
    pub boxy: bool,
}

#[derive(Clone)]
pub struct EdgeIn {
    pub from: usize,
    pub to: usize,
    /// Size of the label across and along the flow.
    pub label: Option<(f64, f64)>,
    /// Length of the heads at the source and at the target, which arrive on a straight line.
    pub heads: (f64, f64),
    /// Edges of the same look are drawn alike and may share a run and a head.
    pub look: usize,
}

#[derive(Clone)]
pub struct GroupIn {
    pub parent: Option<usize>,
    pub nodes: Vec<usize>,
    /// Size of the title across and along the flow.
    pub title: (f64, f64),
}

/// Side of a group box where its title goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TitleSide {
    AlongLow,
    AlongHigh,
    AcrossLow,
}

#[derive(Clone, Copy)]
pub struct Spacing {
    /// Between 2 nodes of a rank.
    pub node: f64,
    /// Between 2 ranks of nodes.
    pub rank: f64,
    /// Between an edge and its neighbour in a rank.
    pub edge: f64,
    /// Between a group border and what it holds.
    pub pad: f64,
    /// Between a group border and what is outside.
    pub outside: f64,
    /// Room for a turn past the side vertex of a fork.
    pub turn: f64,
}

#[derive(Clone)]
pub struct Input {
    /// Whether a layout may place each node under its parent, siblings around the parent's axis.
    pub under_parents: bool,
    pub nodes: Vec<NodeIn>,
    pub edges: Vec<EdgeIn>,
    pub groups: Vec<GroupIn>,
    pub title_side: TitleSide,
    pub spacing: Spacing,
}

/// A piece of an edge path, in the space of the ranks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Seg {
    Move(P),
    Line(P),
    Curve(P, P, P),
}

/// A point: across, along.
pub type P = (f64, f64);

pub struct Output {
    pub nodes: Vec<P>,
    /// Each edge as pieces in the direction of the source edge. A label cuts it in 2 runs.
    pub edges: Vec<Vec<Seg>>,
    pub labels: Vec<Option<P>>,
    /// Box of each group: across low, along low, across high, along high. `None` when empty.
    pub groups: Vec<Option<[f64; 4]>>,
    /// Length of each node along the flow, grown where a side across holds many edges.
    pub along: Vec<f64>,
    /// Half width of each node across the flow, grown where it merges edges.
    pub lo: Vec<f64>,
    /// Whether each edge carries the main path.
    pub main: Vec<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Node(usize),
    Dummy(usize),
    Border(usize, bool),
}

struct Item {
    kind: Kind,
    rank: usize,
    lo: f64,
    hi: f64,
    along: f64,
    group: Option<usize>,
    /// Label of an edge, carried by this dummy.
    label: bool,
    /// A node, or a bend of an edge between 2 neighbouring ranks of nodes.
    short: bool,
}

/// For each edge, the side across the flow it leaves by and the one it arrives by, as the sign of
/// the side, when it uses one.
type Sides = (Vec<Option<f64>>, Vec<Option<f64>>);

/// The paths of the edges and the centres of their labels.
type Routes = (Vec<Vec<Seg>>, Vec<Option<P>>);

/// An order chosen among the candidates: its crossings, its bends, its size, its ranks and its
/// positions.
type Chosen = (usize, usize, f64, Vec<Vec<usize>>, Vec<f64>, (bool, bool));

/// Sweeps of the ordering without a better count of crossings before it stops.
const PATIENCE: usize = 6;
/// Sweeps of the ordering at most.
const SWEEPS: usize = 32;
/// Smallest gap between 2 edges side by side, and between 2 ports on one side of a node:
/// 3 strokes and the half width of an arrow head.
pub const MIN_GAP: f64 = 12.0;
/// Placements at most that learn which links leave by a side across the flow.
const LATERAL_PASSES: usize = 3;
/// Weight of the pull that keeps a side branch next to its parent, against the pulls of links.
const TIE: f64 = 100.0;
/// Long edges up to which every assignment of their lanes to a side is tried.
const MAX_LANE_TRIES: usize = 6;
/// Sweeps of the placement under the parents, downward and upward, the last one downward.
const TOP_DOWN_ROUNDS: usize = 7;
/// Rounds of the positions across the ranks.
const ROUNDS: usize = 60;
/// Flattest angle from the flow of a straight diagonal, in degrees.
const FLATTEST: f64 = 75.0;
/// Radius of the turn of an edge that leaves a node by its side.
const CORNER: f64 = 10.0;
/// Smallest room between a side and a lane for a turn.
const MIN_CORNER: f64 = 4.0;
/// Share of the radius at which the control points of a quarter circle sit.
const ARC: f64 = 0.5523;
/// Widening that an alignment may cost the compact layout.
const WIDER: f64 = 0.5;
/// Room kept between a label and its edge where the edge is cut.
const LABEL_CUT: f64 = 3.0;

/// The edges that close a cycle in a depth-first walk, sources first.
pub(super) fn back_edges(n: usize, edges: &[EdgeIn]) -> Vec<bool> {
    let mut out: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut indegree = vec![0usize; n];
    for (i, e) in edges.iter().enumerate() {
        out[e.from].push(i);
        indegree[e.to] += 1;
    }
    let roots = (0..n)
        .filter(|&v| indegree[v] == 0)
        .chain((0..n).filter(|&v| indegree[v] != 0));
    let mut color = vec![0u8; n];
    let mut back = vec![false; edges.len()];
    for root in roots {
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
            let v = edges[e].to;
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
    back
}

/// Side branches and the main path. For each node with several edges out, the edge toward the
/// longest chain below carries on; when that chain goes on below, up to 2 others that lead to a
/// leaf reached by nothing else stand beside the node, the first after it in its rank and the
/// second before it: `Some(after)` for each such edge. The main path is traced from the source
/// with the longest chain, then the paths of the branches, each stopping at a node that a path
/// before it holds, and a branch path also at a merge.
pub(super) fn main_path(
    n: usize,
    edges: &[EdgeIn],
    oriented: &[(usize, usize)],
    back: &[bool],
) -> (Vec<Option<bool>>, Vec<bool>) {
    let mut succ: Vec<Vec<usize>> = vec![Vec::new(); n];
    let (mut ins, mut outs) = (vec![0usize; n], vec![0usize; n]);
    for (e, &(u, v)) in oriented.iter().enumerate() {
        succ[u].push(v);
        ins[edges[e].to] += 1;
        outs[edges[e].from] += 1;
    }
    // Longest chain below each node, over the edges turned to break the cycles.
    let mut depth = vec![usize::MAX; n];
    fn walk(v: usize, succ: &[Vec<usize>], depth: &mut [usize]) -> usize {
        if depth[v] != usize::MAX {
            return depth[v];
        }
        depth[v] = 0;
        let d = succ[v]
            .iter()
            .map(|&w| walk(w, succ, depth) + 1)
            .max()
            .unwrap_or(0);
        depth[v] = d;
        d
    }
    for v in 0..n {
        walk(v, &succ, &mut depth);
    }
    let mut out = vec![None; oriented.len()];
    let forward = |u: usize| -> Vec<usize> {
        (0..oriented.len())
            .filter(|&e| !back[e] && oriented[e].0 == u)
            .collect()
    };
    let deepest = |u: usize| {
        forward(u)
            .into_iter()
            .max_by_key(|&e| (depth[oriented[e].1], e))
    };
    for u in 0..n {
        let Some(spine) = deepest(u) else { continue };
        // Branches stand aside only from a path that goes on below.
        if forward(u).len() < 2 || depth[oriented[spine].1] == 0 {
            continue;
        }
        let mut given = 0;
        for e in forward(u) {
            let t = oriented[e].1;
            if e == spine || given == 2 || ins[t] != 1 || outs[t] != 0 {
                continue;
            }
            out[e] = Some(given == 0);
            given += 1;
        }
    }
    // The main path, traced from the source with the longest chain below, then the paths of the
    // other branches; a path stops at a node that one before it holds.
    let mut main = vec![false; oriented.len()];
    let mut held = vec![false; n];
    let mut has_pred = vec![false; n];
    for (e, &(_, v)) in oriented.iter().enumerate() {
        if !back[e] {
            has_pred[v] = true;
        }
    }
    let mut starts: Vec<usize> = (0..n).filter(|&v| !has_pred[v]).collect();
    starts.sort_by_key(|&v| (std::cmp::Reverse(depth[v]), v));
    // Sources of the same depth that meet in one node first are equals: the main path starts at
    // that node, and they stand around it.
    if let Some(&top) = starts.first()
        && let Some(e) = deepest(top)
    {
        let meet = oriented[e].1;
        let equals = starts
            .iter()
            .filter(|&&s| {
                depth[s] == depth[top] && deepest(s).is_some_and(|f| oriented[f].1 == meet)
            })
            .count();
        if equals > 1 {
            starts.insert(0, meet);
        }
    }
    let mut queue: std::collections::VecDeque<usize> = starts.into_iter().collect();
    let mut first = true;
    while let Some(start) = queue.pop_front() {
        if held[start] {
            continue;
        }
        held[start] = true;
        let mut at = start;
        while let Some(e) = deepest(at) {
            for f in forward(at) {
                if f != e && out[f].is_none() {
                    queue.push_back(oriented[f].1);
                }
            }
            let t = oriented[e].1;
            // A path other than the main one stops at a merge, which stays at the median.
            if held[t] || !first && ins[t] > 1 {
                break;
            }
            main[e] = true;
            held[t] = true;
            at = t;
        }
        first = false;
    }
    (out, main)
}

/// The key of each group in the order of a walk: the first visit of its nodes, 0 when empty.
fn group_keys(groups: &[GroupIn], visit: &[usize]) -> Vec<f64> {
    groups
        .iter()
        .map(|g| {
            g.nodes
                .iter()
                .map(|&v| visit[v] as f64)
                .fold(f64::INFINITY, f64::min)
        })
        .map(|k| if k.is_finite() { k } else { 0.0 })
        .collect()
}

/// Rank of each node on the longest path, a source pulled down next to its first successor.
fn ranks(n: usize, oriented: &[(usize, usize)]) -> Vec<usize> {
    let mut indegree = vec![0usize; n];
    let mut succ: Vec<Vec<usize>> = vec![Vec::new(); n];
    for &(u, v) in oriented {
        indegree[v] += 1;
        succ[u].push(v);
    }
    let mut order = Vec::with_capacity(n);
    let mut left = indegree.clone();
    let mut ready: Vec<usize> = (0..n).rev().filter(|&v| left[v] == 0).collect();
    while let Some(u) = ready.pop() {
        order.push(u);
        for &v in succ[u].iter().rev() {
            left[v] -= 1;
            if left[v] == 0 {
                ready.push(v);
            }
        }
    }
    let mut rank = vec![0i64; n];
    for &u in &order {
        for &v in &succ[u] {
            rank[v] = rank[v].max(rank[u] + 1);
        }
    }
    for &u in order.iter().rev() {
        if indegree[u] == 0 && !succ[u].is_empty() {
            rank[u] = succ[u].iter().map(|&v| rank[v]).min().unwrap_or(1) - 1;
        }
    }
    let low = rank.iter().copied().min().unwrap_or(0);
    rank.iter()
        .map(|&r| usize::try_from(r - low).unwrap_or(0))
        .collect()
}

/// Groups from `g` up to the outermost.
fn ancestry(groups: &[GroupIn], g: Option<usize>) -> Vec<usize> {
    let mut chain = Vec::new();
    let mut at = g;
    while let Some(c) = at {
        chain.push(c);
        at = groups[c].parent;
    }
    chain
}

struct Layout<'a> {
    input: &'a Input,
    items: Vec<Item>,
    /// Items of each rank, in order.
    order: Vec<Vec<usize>>,
    /// Items linked to each item in the rank before and in the rank after.
    up: Vec<Vec<usize>>,
    down: Vec<Vec<usize>>,
    /// Groups from each group up to the outermost.
    chains: Vec<Vec<usize>>,
    /// Ranks that a group spans, `None` for a group without nodes.
    spans: Vec<Option<(usize, usize)>>,
    /// Links that leave or reach a node by a side across the flow, both ways: they take no
    /// port on the sides along the flow and ask no position.
    lateral: std::cell::RefCell<std::collections::BTreeSet<(usize, usize)>>,
    /// Side branches that stand beside their parent in its rank: parent, branch, the room
    /// between them, and whether the branch stands after the parent in the rank.
    flat: Vec<(usize, usize, f64, bool)>,
    /// The items of each edge of the main path, from its upper end.
    spine: Vec<Vec<usize>>,
}

impl Layout<'_> {
    fn chain_of(&self, item: usize) -> &[usize] {
        match self.items[item].group {
            Some(g) => &self.chains[g],
            None => &[],
        }
    }

    fn link(&mut self, a: usize, b: usize) {
        self.down[a].push(b);
        self.up[b].push(a);
    }

    /// Whether nothing of rank `r` but `skip`, the group borders and the thin bends on the lane
    /// lies across between `a` and half an edge gap past `lane`, half an edge gap kept.
    fn clear(&self, x: &[f64], r: usize, a: f64, lane: f64, skip: usize) -> bool {
        let edge = self.input.spacing.edge;
        let b = lane + (lane - a).signum() * edge / 2.0;
        let (lo, hi) = (a.min(b), a.max(b));
        self.order[r].iter().all(|&j| {
            let item = &self.items[j];
            let on_lane =
                matches!(item.kind, Kind::Dummy(_)) && !item.label && (x[j] - lane).abs() < 0.05;
            j == skip
                || on_lane
                || matches!(item.kind, Kind::Border(..))
                || x[j] + self.items[j].hi + edge / 2.0 <= lo
                || x[j] - self.items[j].lo - edge / 2.0 >= hi
        })
    }

    /// Whether the items `inner` of a lane fit at `to` across between their neighbours at `x`.
    fn room(&self, x: &[f64], inner: &[usize], to: f64) -> bool {
        inner.iter().all(|&i| {
            let rank = &self.order[self.items[i].rank];
            let p = rank.iter().position(|&j| j == i).unwrap_or(0);
            let left = p == 0 || to >= x[rank[p - 1]] + self.separation(rank[p - 1], i) - 1e-6;
            let right = p + 1 >= rank.len()
                || to <= x[rank[p + 1]] - self.separation(i, rank[p + 1]) + 1e-6;
            left && right
        })
    }

    /// Room that the gaps between ranks take.
    fn gap_room(&self) -> f64 {
        self.input.spacing.rank / 2.0 * self.order.len() as f64
    }

    /// Smallest distance between the centres of `a` and of `b` when `b` follows `a` in a rank.
    fn separation(&self, a: usize, b: usize) -> f64 {
        let sp = &self.input.spacing;
        let (ia, ib) = (&self.items[a], &self.items[b]);
        if let Some(&(_, _, room, _)) = self
            .flat
            .iter()
            .find(|f| (f.0, f.1) == (a, b) || (f.1, f.0) == (a, b))
        {
            return ia.hi + ib.lo + room;
        }
        let thin = |i: &Item| matches!(i.kind, Kind::Dummy(_)) && !i.label;
        let gap = match (ia.kind, ib.kind) {
            (Kind::Border(g, false), _) => {
                let title = if self.input.title_side == TitleSide::AcrossLow {
                    self.input.groups[g].title.0
                } else {
                    0.0
                };
                sp.pad + title
            }
            (_, Kind::Border(_, true)) => sp.pad,
            (Kind::Border(_, true), Kind::Border(_, false)) => sp.outside,
            (Kind::Border(..), _) | (_, Kind::Border(..)) => sp.outside,
            _ if thin(ia) && thin(ib) => sp.edge,
            _ if thin(ia) || thin(ib) => sp.edge * 1.5,
            _ if ia.label || ib.label => sp.node * 0.6,
            _ => sp.node,
        };
        ia.hi + ib.lo + gap
    }

    fn border(&self, g: usize, right: bool) -> Option<usize> {
        (0..self.items.len()).find(|&i| self.items[i].kind == Kind::Border(g, right))
    }
}

/// What the layout decides before it draws: the edges turned around to break the cycles, the rank
/// of each node, and the position of each node across the ranks.
pub struct Plan {
    pub back: Vec<bool>,
    pub rank: Vec<usize>,
    pub across: Vec<f64>,
    /// Whether each edge carries the main path.
    pub main: Vec<bool>,
}

/// The plan of `input`. `None` when it has no node.
pub fn plan(input: &Input) -> Option<Plan> {
    full(input, false).map(|(plan, _)| plan)
}

/// Lays out `input`. `None` when it has no node.
pub fn layout(input: &Input) -> Option<Output> {
    full(input, true).and_then(|(_, out)| out)
}

/// The plan of `input`, and with `draw` the drawing that follows from it.
fn full(input: &Input, draw: bool) -> Option<(Plan, Option<Output>)> {
    let n = input.nodes.len();
    if n == 0 {
        return None;
    }
    let back = back_edges(n, &input.edges);
    let oriented: Vec<(usize, usize)> = input
        .edges
        .iter()
        .zip(&back)
        .map(|(e, &b)| if b { (e.to, e.from) } else { (e.from, e.to) })
        .collect();
    let mut rank: Vec<usize> = ranks(n, &oriented).iter().map(|r| r * 2).collect();
    let (flat, main) = main_path(n, &input.edges, &oriented, &back);
    for (e, side) in flat.iter().enumerate() {
        if side.is_some() {
            rank[oriented[e].1] = rank[oriented[e].0];
        }
    }
    let chains: Vec<Vec<usize>> = (0..input.groups.len())
        .map(|g| ancestry(&input.groups, Some(g)))
        .collect();
    let mut node_group = vec![None; n];
    for (g, group) in input.groups.iter().enumerate() {
        for &v in &group.nodes {
            node_group[v] = Some(g);
        }
    }
    let mut spans: Vec<Option<(usize, usize)>> = vec![None; input.groups.len()];
    for v in 0..n {
        for &g in &ancestry(&input.groups, node_group[v]) {
            let s = spans[g].get_or_insert((rank[v], rank[v]));
            *s = (s.0.min(rank[v]), s.1.max(rank[v]));
        }
    }
    let depth = rank.iter().max().copied().unwrap_or(0) + 1;
    let mut lay = Layout {
        input,
        items: Vec::new(),
        order: vec![Vec::new(); depth],
        up: Vec::new(),
        down: Vec::new(),
        chains,
        spans,
        lateral: Default::default(),
        flat: Vec::new(),
        spine: Vec::new(),
    };
    let mut node_item = vec![0; n];
    for (v, node) in input.nodes.iter().enumerate() {
        node_item[v] = lay.items.len();
        lay.items.push(Item {
            kind: Kind::Node(v),
            rank: rank[v],
            lo: node.lo,
            hi: node.hi,
            along: node.along,
            group: node_group[v],
            label: false,
            short: true,
        });
    }
    let depths: Vec<usize> = lay.chains.iter().map(Vec::len).collect();
    // Items of each edge from its upper end to its lower end.
    let mut edge_items: Vec<Vec<usize>> = Vec::new();
    for (e, &(u, v)) in oriented.iter().enumerate() {
        let (ru, rv) = (rank[u], rank[v]);
        let mid = (ru + rv) / 2;
        let label_rank = if mid % 2 == 1 {
            mid
        } else {
            mid.saturating_sub(1)
        };
        let mut list = vec![node_item[u]];
        for r in ru + 1..rv {
            // The deepest group that holds the bend: one of the source above its last rank, or
            // one of the target below its first rank.
            let from = ancestry(&input.groups, node_group[u])
                .into_iter()
                .filter(|&g| lay.spans[g].is_some_and(|s| r <= s.1));
            let to = ancestry(&input.groups, node_group[v])
                .into_iter()
                .filter(|&g| lay.spans[g].is_some_and(|s| r >= s.0));
            let group = from.chain(to).max_by_key(|&g| (depths[g], usize::MAX - g));
            let label = input.edges[e].label.filter(|_| r == label_rank);
            let (across, along) = label.unwrap_or((0.0, 0.0));
            list.push(lay.items.len());
            lay.items.push(Item {
                kind: Kind::Dummy(e),
                rank: r,
                lo: across / 2.0,
                hi: across / 2.0,
                along,
                group,
                label: label.is_some(),
                short: rv - ru == 2,
            });
        }
        list.push(node_item[v]);
        edge_items.push(list);
    }
    for g in 0..input.groups.len() {
        if let Some((lo, hi)) = lay.spans[g] {
            for r in lo..=hi {
                for right in [false, true] {
                    lay.items.push(Item {
                        kind: Kind::Border(g, right),
                        rank: r,
                        lo: 0.0,
                        hi: 0.0,
                        along: 0.0,
                        group: Some(g),
                        label: false,
                        short: true,
                    });
                }
            }
        }
    }
    let count = lay.items.len();
    lay.up = vec![Vec::new(); count];
    lay.down = vec![Vec::new(); count];
    lay.spine = (0..edge_items.len())
        .filter(|&e| main[e])
        .map(|e| edge_items[e].clone())
        .collect();
    lay.spine.sort_by_key(|list| lay.items[list[0]].rank);
    for (e, list) in edge_items.iter().enumerate() {
        if let Some(after) = flat[e] {
            let room = input.edges[e].label.map_or(input.spacing.node, |l| {
                (l.0 + 2.0 * input.spacing.edge).max(input.spacing.node)
            });
            lay.flat.push((list[0], list[1], room, after));
            continue;
        }
        for w in list.windows(2) {
            lay.link(w[0], w[1]);
        }
    }
    for g in 0..input.groups.len() {
        for right in [false, true] {
            let chain: Vec<usize> = (0..count)
                .filter(|&i| lay.items[i].kind == Kind::Border(g, right))
                .collect();
            for w in chain.windows(2) {
                lay.link(w[0], w[1]);
            }
        }
    }
    // First orders: walks from the sources, depth first both ways and breadth first, and the
    // order of the source. Each is improved from both ends; the fewest crossings win, then the
    // fewest bends, then the narrowest.
    let mut succ: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut has_pred = vec![false; n];
    for &(u, v) in &oriented {
        succ[u].push(v);
        has_pred[v] = true;
    }
    let roots: Vec<usize> = (0..n)
        .filter(|&v| !has_pred[v])
        .chain((0..n).filter(|&v| has_pred[v]))
        .collect();
    let walk = |reversed: bool, breadth: bool| -> Vec<usize> {
        let mut visit = vec![usize::MAX; n];
        let mut counter = 0;
        for &root in &roots {
            let mut queue = std::collections::VecDeque::from([root]);
            while let Some(u) = if breadth {
                queue.pop_front()
            } else {
                queue.pop_back()
            } {
                if visit[u] != usize::MAX {
                    continue;
                }
                visit[u] = counter;
                counter += 1;
                let next: Vec<usize> = if reversed == breadth {
                    succ[u].iter().rev().copied().collect()
                } else {
                    succ[u].clone()
                };
                queue.extend(next);
            }
        }
        visit
    };
    let starts: Vec<Vec<usize>> = vec![
        walk(false, false),
        walk(true, false),
        walk(false, true),
        (0..n).collect(),
    ];
    let mut start: Vec<Vec<usize>> = vec![Vec::new(); depth];
    for i in 0..count {
        start[lay.items[i].rank].push(i);
    }
    // The first candidate is the reference: the first walk, sweeps without exchanges, bends
    // lined up as they can.
    let mut candidates: Vec<(usize, Vec<Vec<usize>>, bool)> = Vec::new();
    for (k, visit) in starts.iter().enumerate() {
        let keys: Vec<f64> = lay
            .items
            .iter()
            .map(|item| match item.kind {
                Kind::Node(v) => visit[v] as f64,
                Kind::Dummy(e) => visit[oriented[e].0] as f64 + 0.5 + e as f64 * 1e-6,
                Kind::Border(..) => 0.0,
            })
            .collect();
        let gkey = group_keys(&input.groups, visit);
        let runs: &[(bool, bool)] = if k == 0 {
            &[(true, false), (true, true), (false, true)]
        } else {
            &[(true, true), (false, true)]
        };
        for &(first_down, exchange) in runs {
            for (r, items) in start.iter().enumerate() {
                lay.order[r] = lay.arrange(None, items, &|i| Some(keys[i]), &gkey);
            }
            let crossings = lay.improve(first_down, exchange);
            if candidates.is_empty() {
                candidates.push((crossings, lay.order.clone(), false));
            }
            if !candidates.iter().skip(1).any(|c| c.1 == lay.order) {
                candidates.push((crossings, lay.order.clone(), true));
            }
        }
    }
    // The lanes of the long edges, each on the left or on the right of the nodes it runs past:
    // every assignment is a start when there are few of them.
    let long: Vec<usize> = (0..oriented.len())
        .filter(|&e| rank[oriented[e].1] - rank[oriented[e].0] > 2)
        .collect();
    if !long.is_empty() && long.len() <= MAX_LANE_TRIES {
        let visit = &starts[0];
        let gkey = group_keys(&input.groups, visit);
        let far = n as f64 + 1.0;
        for mask in 0..(1usize << long.len()) {
            let keys: Vec<f64> = lay
                .items
                .iter()
                .map(|item| match item.kind {
                    Kind::Node(v) => visit[v] as f64,
                    Kind::Dummy(e) => match long.iter().position(|&l| l == e) {
                        Some(k) if mask >> k & 1 == 1 => far + visit[oriented[e].0] as f64,
                        Some(_) => -far + visit[oriented[e].0] as f64,
                        None => visit[oriented[e].0] as f64 + 0.5 + e as f64 * 1e-6,
                    },
                    Kind::Border(..) => 0.0,
                })
                .collect();
            for (r, items) in start.iter().enumerate() {
                lay.order[r] = lay.arrange(None, items, &|i| Some(keys[i]), &gkey);
            }
            let crossings = lay.improve(true, true);
            if !candidates.iter().skip(1).any(|c| c.1 == lay.order) {
                candidates.push((crossings, lay.order.clone(), true));
            }
        }
    }
    // Each order also tries the main path in the middle of its forks.
    for k in 0..candidates.len() {
        lay.order = candidates[k].1.clone();
        lay.balance();
        if !candidates.iter().any(|c| c.1 == lay.order) {
            let crossings = lay.crossings();
            candidates.push((crossings, lay.order.clone(), true));
        }
    }
    // The size is the area: the width across the ranks times the bands and the gaps along.
    let mut bands = vec![0.0f64; depth];
    for item in &lay.items {
        bands[item.rank] = bands[item.rank].max(item.along);
    }
    let band_sum: f64 = bands.iter().sum();
    let size = |lay: &Layout, x: &[f64]| {
        let hi = (0..count)
            .map(|i| x[i] + lay.items[i].hi)
            .fold(f64::NEG_INFINITY, f64::max);
        let lo = (0..count)
            .map(|i| x[i] - lay.items[i].lo)
            .fold(f64::INFINITY, f64::min);
        (hi - lo) * (band_sum + lay.gap_room())
    };
    // The first candidate is the reference. Another wins with fewer crossings, then fewer bends,
    // then a smaller area. A larger area is fine.
    let mut best: Option<Chosen> = None;
    for (crossings, order, align) in candidates {
        lay.order = order;
        // The vector drawing places nodes under their parents; the grid keeps the mean.
        let modes: &[bool] = if align && input.under_parents {
            &[true]
        } else {
            &[false]
        };
        for &top_down in modes {
            let x = lay.across(align, top_down);
            let bends = lay.bends(&x);
            let area = size(&lay, &x);
            let better = best.as_ref().is_none_or(|b| {
                (crossings, bends) < (b.0, b.1)
                    || (crossings, bends) == (b.0, b.1) && area < b.2 * (1.0 - 1e-9)
            });
            if better {
                best = Some((
                    crossings,
                    bends,
                    area,
                    lay.order.clone(),
                    x,
                    (align, top_down),
                ));
            }
        }
    }
    let (_, _, _, order, x, (align, top_down)) = best?;
    lay.order = order;

    let sp = input.spacing;
    // Side exits: an edge whose lane runs beyond a side of its node leaves from that side, or
    // arrives there, when nothing of the rank lies between the node and the lane: 1 turn instead
    // of a step of 2. A side takes edges that leave or edges that arrive, not both; the edges
    // that leave are served first, so that a fan opens on both sides.
    let clear =
        |x: &[f64], r: usize, a: f64, lane: f64, skip: usize| lay.clear(x, r, a, lane, skip);
    // Head lengths at the upper and the lower end of each edge, in the order of the layout.
    let heads: Vec<(f64, f64)> = input
        .edges
        .iter()
        .zip(&back)
        .map(|(e, &b)| if b { (e.heads.1, e.heads.0) } else { e.heads })
        .collect();
    // The side exits and arrivals, given out to the edges that leave first, or edge by edge.
    let exits = |x: &[f64], leaving_first: bool| -> Sides {
        let mut side_out: Vec<Option<f64>> = vec![None; input.edges.len()];
        let mut side_in: Vec<Option<f64>> = vec![None; input.edges.len()];
        // The sides that the side branches take, whole.
        let mut taken: Vec<(usize, bool, bool)> = Vec::new();
        let mut branches: Vec<(usize, bool)> = Vec::new();
        for (e, side) in flat.iter().enumerate() {
            if let Some(after) = *side {
                branches.push((oriented[e].0, after));
                branches.push((oriented[e].1, !after));
            }
        }
        // The edge whose lane lies furthest out takes a side first.
        let reach_of = |e: usize, leaving: bool| {
            let list = &edge_items[e];
            let (v, lane) = if leaving {
                (oriented[e].0, list[1])
            } else {
                (oriented[e].1, list[list.len() - 2])
            };
            (x[lane] - x[node_item[v]]).abs()
        };
        let mut order: Vec<(usize, bool)> = (0..edge_items.len())
            .flat_map(|e| [(e, true), (e, false)])
            .collect();
        order.sort_by(|a, b| {
            let first = |o: &(usize, bool)| leaving_first && !o.1;
            first(a)
                .cmp(&first(b))
                .then(reach_of(b.0, b.1).total_cmp(&reach_of(a.0, a.1)))
                .then(a.cmp(b))
        });
        for (e, leaving) in order {
            if flat[e].is_some() {
                continue;
            }
            let list = &edge_items[e];
            let (v, lane, head) = if leaving {
                (oriented[e].0, list[1], heads[e].0)
            } else {
                (oriented[e].1, list[list.len() - 2], heads[e].1)
            };
            let node = &input.nodes[v];
            let dx = x[lane] - x[node_item[v]];
            let sign = dx.signum();
            let right = sign > 0.0;
            if dx.abs() + 1e-6 < node.lo + MIN_CORNER + head
                || right && node.hi > node.lo + 0.5
                || taken.contains(&(v, right, !leaving))
                || branches.contains(&(v, right))
                || !clear(
                    x,
                    rank[v],
                    x[node_item[v]] + sign * node.lo,
                    x[lane],
                    node_item[v],
                )
            {
                continue;
            }
            taken.push((v, right, leaving));
            if leaving {
                side_out[e] = Some(sign);
            } else {
                side_in[e] = Some(sign);
            }
        }
        (side_out, side_in)
    };
    // More placements, each knowing which links the one before sent by a side across the flow,
    // until those links stay the same.
    let mut x = x;
    for _ in 0..LATERAL_PASSES {
        let first = exits(&x, true);
        let mut links = std::collections::BTreeSet::new();
        for (e, list) in edge_items.iter().enumerate() {
            if first.0[e].is_some() {
                links.insert((list[0], list[1]));
                links.insert((list[1], list[0]));
            }
            if first.1[e].is_some() {
                let (a, b) = (list[list.len() - 1], list[list.len() - 2]);
                links.insert((a, b));
                links.insert((b, a));
            }
        }
        if *lay.lateral.borrow() == links {
            break;
        }
        *lay.lateral.borrow_mut() = links;
        x = lay.across(align, top_down);
    }
    // The lanes of long edges that run inside the reach of a side of a node they end at, moved
    // out past that
    // side where their ranks leave room, so that the edge may turn once into the side instead
    // of stepping: the routes compare both.
    let mut pushed = x.clone();
    for (e, list) in edge_items.iter().enumerate() {
        let inner = &list[1..list.len() - 1];
        if flat[e].is_some()
            || inner.len() < 2
            || inner.iter().any(|&i| (x[i] - x[inner[0]]).abs() > 0.05)
        {
            continue;
        }
        let (u, v) = oriented[e];
        for (w, head) in [(v, heads[e].1), (u, heads[e].0)] {
            let node = &input.nodes[w];
            let c = x[node_item[w]];
            let l = pushed[inner[0]];
            let dx = l - c;
            let need = node.lo + CORNER + head;
            if dx.abs() < 0.05 || dx.abs() >= node.lo + MIN_CORNER + head {
                continue;
            }
            let to = c + dx.signum() * need;
            if lay.room(&pushed, inner, to) {
                for &i in inner {
                    pushed[i] = to;
                }
            }
        }
    }
    // Lanes of the same look that reach one node side by side join: the lane of the edge that
    // runs over fewer ranks moves onto the other one, when nothing lies between them in its
    // ranks nor between its source and the joined lane, and no label rides on either.
    let join = |xv: &mut Vec<f64>| {
        let lane_of = |e: usize| {
            let list = &edge_items[e];
            let inner = &list[1..list.len() - 1];
            let straight = flat[e].is_none()
                && !back[e]
                && !main[e]
                && !inner.is_empty()
                && inner
                    .iter()
                    .all(|&i| (xv[i] - xv[inner[0]]).abs() < 0.05 && !lay.items[i].label);
            straight.then_some(inner)
        };
        let mut lanes: Vec<(usize, &[usize])> = (0..edge_items.len())
            .filter_map(|e| lane_of(e).map(|inner| (e, inner)))
            .collect();
        lanes.sort_by_key(|&(e, inner)| (std::cmp::Reverse(inner.len()), e));
        for k in 0..lanes.len() {
            let (f, short) = lanes[k];
            let (uf, vf) = oriented[f];
            let partner = lanes[..k].iter().find(|&&(e, long)| {
                oriented[e].1 == vf
                    && input.edges[e].look == input.edges[f].look
                    && long.len() > short.len()
                    && (xv[long[0]] - xv[short[0]]).abs() > 0.05
                    && short.iter().all(|&i| {
                        let r = lay.items[i].rank;
                        let Some(&j) = long.iter().find(|&&j| lay.items[j].rank == r) else {
                            return false;
                        };
                        let order = &lay.order[r];
                        let (pi, pj) = (
                            order.iter().position(|&o| o == i),
                            order.iter().position(|&o| o == j),
                        );
                        matches!((pi, pj), (Some(a), Some(b)) if a.abs_diff(b) == 1)
                    })
            });
            let Some(&(_, long)) = partner else { continue };
            let to = xv[long[0]];
            let c = xv[node_item[uf]];
            let node = &input.nodes[uf];
            let sign = (to - c).signum();
            let off = (xv[short[0]] - c).abs() > node.lo;
            if off
                && (to - c).abs() > node.lo
                && lay.clear(xv, rank[uf], c + sign * node.lo, to, node_item[uf])
            {
                for &i in short {
                    xv[i] = to;
                }
            }
        }
    };
    join(&mut x);
    join(&mut pushed);
    let mut variants: Vec<(Vec<f64>, Sides)> = [true, false]
        .into_iter()
        .map(|l| (x.clone(), exits(&x, l)))
        .collect();
    if pushed != x {
        for l in [true, false] {
            variants.push((pushed.clone(), exits(&pushed, l)));
        }
    }
    // A side across the flow that holds n edges is at least (n + 1) gaps long: the node grows
    // along the flow when it is shorter.
    let mut along_of: Vec<f64> = input.nodes.iter().map(|v| v.along).collect();
    for (_, (side_out, side_in)) in &variants {
        let mut count: BTreeMap<(usize, bool), usize> = BTreeMap::new();
        for e in 0..edge_items.len() {
            if let Some(sign) = side_out[e] {
                *count.entry((oriented[e].0, sign > 0.0)).or_default() += 1;
            }
            if let Some(sign) = side_in[e] {
                *count.entry((oriented[e].1, sign > 0.0)).or_default() += 1;
            }
        }
        for (&(v, _), &k) in &count {
            along_of[v] = along_of[v].max((k + 1) as f64 * MIN_GAP);
        }
    }
    // The boxes of text of a rank share the length of the longest along the flow.
    let mut longest = vec![0.0f64; depth];
    for v in (0..n).filter(|&v| input.nodes[v].boxy) {
        longest[rank[v]] = longest[rank[v]].max(along_of[v]);
    }
    for v in (0..n).filter(|&v| input.nodes[v].boxy) {
        along_of[v] = longest[rank[v]];
    }
    for v in 0..n {
        lay.items[node_item[v]].along = along_of[v];
    }

    // Positions along the ranks.
    let mut band = vec![0.0f64; depth];
    for item in &lay.items {
        band[item.rank] = band[item.rank].max(item.along);
    }
    let groups = &input.groups;
    let title_along = |g: usize| match input.title_side {
        TitleSide::AcrossLow => 0.0,
        _ => groups[g].title.1,
    };
    // Room above and below a rank for the group boxes that start or end on it, outer last.
    let mut stack_low = vec![0.0f64; groups.len()];
    let mut stack_high = vec![0.0f64; groups.len()];
    for g in (0..groups.len()).rev() {
        let Some((lo, hi)) = lay.spans[g] else {
            continue;
        };
        let (own_low, own_high) = match input.title_side {
            TitleSide::AlongLow => (sp.pad + title_along(g), sp.pad),
            TitleSide::AlongHigh => (sp.pad, sp.pad + title_along(g)),
            TitleSide::AcrossLow => (sp.pad, sp.pad),
        };
        let children = (0..groups.len()).filter(|&c| groups[c].parent == Some(g));
        let (mut inner_low, mut inner_high) = (0.0f64, 0.0f64);
        for c in children {
            if let Some((clo, chi)) = lay.spans[c] {
                if clo == lo {
                    inner_low = inner_low.max(stack_low[c]);
                }
                if chi == hi {
                    inner_high = inner_high.max(stack_high[c]);
                }
            }
        }
        stack_low[g] = own_low + inner_low;
        stack_high[g] = own_high + inner_high;
    }
    let mut need_low = vec![0.0f64; depth];
    let mut need_high = vec![0.0f64; depth];
    for g in 0..groups.len() {
        if let Some((lo, hi)) = lay.spans[g] {
            need_low[lo] = need_low[lo].max(stack_low[g]);
            need_high[hi] = need_high[hi].max(stack_high[g]);
        }
    }
    // A title across a box that spans one rank widens that rank.
    if input.title_side == TitleSide::AcrossLow {
        for g in (0..groups.len()).rev() {
            if let Some((lo, hi)) = lay.spans[g] {
                let have: f64 = (lo..=hi).map(|r| band[r]).sum::<f64>()
                    + (hi - lo) as f64 * sp.rank / 2.0
                    + stack_low[g]
                    + stack_high[g];
                let want = groups[g].title.1 + 2.0 * sp.pad;
                if have < want {
                    band[lo] += want - have;
                }
            }
        }
    }
    // Height of the gap after each rank: half the rank spacing, more where square steps need
    // separate runs across, 1 port gap apart.
    let half = sp.rank / 2.0;
    let mut gap = vec![half; depth];
    let mut guess = Vec::new();
    for (a, links) in lay.down.iter().enumerate() {
        if matches!(lay.items[a].kind, Kind::Border(..)) {
            continue;
        }
        let r = lay.items[a].rank as f64;
        for &b in links {
            if (x[b] - x[a]).abs() >= 0.5 {
                guess.push(Step {
                    edge: a,
                    step: b,
                    from: x[a],
                    to: x[b],
                    z0: r,
                    z1: r + 1.0,
                });
            }
        }
    }
    for (t, (_, count)) in guess.iter().zip(square_levels(&guess)) {
        let r = lay.items[t.edge].rank;
        gap[r] = gap[r].max((count as f64 + 1.0) * MIN_GAP);
    }
    let mut pos = vec![0.0f64; depth];
    pos[0] = need_low[0] + band[0] / 2.0;
    for r in 1..depth {
        pos[r] = pos[r - 1]
            + band[r - 1] / 2.0
            + need_high[r - 1]
            + gap[r - 1]
            + need_low[r]
            + band[r] / 2.0;
    }
    let zone = |r: usize| -> (f64, f64) {
        // Where edges may bend between rank `r` and the next.
        let from = pos[r] + band[r] / 2.0 + need_high[r];
        (from, from + gap[r])
    };

    let nodes: Vec<P> = (0..n).map(|v| (x[node_item[v]], pos[rank[v]])).collect();
    let plan = Plan {
        rank: rank.iter().map(|r| r / 2).collect(),
        across: nodes.iter().map(|p| p.0).collect(),
        back: back.clone(),
        main: main.clone(),
    };
    if !draw {
        return Some((plan, None));
    }
    // Ports: where each edge leaves its upper node and reaches its lower node, spread across the
    // side in the order of what they lead to.
    let mut ends: Vec<Vec<(usize, bool)>> = vec![Vec::new(); n];
    for e in (0..edge_items.len()).filter(|&e| flat[e].is_none()) {
        ends[oriented[e].0].push((e, true));
        ends[oriented[e].1].push((e, false));
    }
    // The ports and the routes, with the ports shared evenly by default or placed to keep the
    // edges straight. The routes with fewer crossings, then fewer bends, then shorter win.
    // Where an end of an edge aims across: the first bend the route passes through, or, when
    // it meets the other node first, the middle of what both sides can reach, else that node.
    let aim = |e: usize, leaving: bool| -> f64 {
        let list = &edge_items[e];
        let inner = &list[1..list.len() - 1];
        let hard = |i: &&usize| lay.items[**i].label || band[lay.items[**i].rank] > 0.0;
        let bend = if leaving {
            inner.iter().find(hard)
        } else {
            inner.iter().rev().find(hard)
        };
        if let Some(&i) = bend {
            return x[i];
        }
        let (a, b) = (list[0], list[list.len() - 1]);
        let (a, b) = if leaving { (a, b) } else { (b, a) };
        // An end alone on its side stays at its middle.
        let (ra, rb) = if leaving {
            (lay.end_slack(a, true), lay.end_slack(b, false))
        } else {
            (lay.end_slack(a, false), lay.end_slack(b, true))
        };
        let lo = (x[a] - ra).max(x[b] - rb);
        let hi = (x[a] + ra).min(x[b] + rb);
        if lo <= hi { (lo + hi) / 2.0 } else { x[b] }
    };
    let router = route::Router {
        input,
        lay: &lay,
        edge_items: &edge_items,
        oriented: &oriented,
        flat: &flat,
        back: &back,
        node_item: &node_item,
        ends: &ends,
        along_of: &along_of,
        nodes: &nodes,
        pos: &pos,
        band: &band,
        heads: &heads,
        rank: &rank,
        zone: &zone,
        aim: &aim,
    };
    let route_with = |x: &[f64], side_out: &[Option<f64>], side_in: &[Option<f64>]| -> Routes {
        router.routes(x, side_out, side_in)
    };
    let (edges, labels) = {
        let mut best: Option<((usize, usize, u64), Routes)> = None;
        for (xv, (side_out, side_in)) in &variants {
            let routes = route_with(xv, side_out, side_in);
            let cost = route_cost(&routes.0);
            if best.as_ref().is_none_or(|b| cost < b.0) {
                best = Some((cost, routes));
            }
        }
        best?.1
    };
    let boxes = (0..groups.len())
        .map(|g| {
            let (lo, hi) = lay.spans[g]?;
            let l = lay.border(g, false)?;
            let r = lay.border(g, true)?;
            Some([
                x[l],
                pos[lo] - band[lo] / 2.0 - stack_low[g],
                x[r],
                pos[hi] + band[hi] / 2.0 + stack_high[g],
            ])
        })
        .collect();
    let out = Output {
        nodes,
        edges,
        labels,
        groups: boxes,
        along: along_of,
        lo: input.nodes.iter().map(|v| v.lo).collect(),
        main,
    };
    Some((plan, Some(out)))
}

/// Crossings, turns and length of a set of edge paths, to compare 2 routings: a step across makes
/// 2 turns, a quarter turn 1.
fn route_cost(paths: &[Vec<Seg>]) -> (usize, usize, u64) {
    let runs: Vec<Vec<Vec<P>>> = paths.iter().map(|p| polylines(p)).collect();
    let mut crossings = 0;
    for (i, a) in runs.iter().enumerate() {
        for b in &runs[i + 1..] {
            for sa in a.iter().flat_map(|r| r.windows(2)) {
                for sb in b.iter().flat_map(|r| r.windows(2)) {
                    if crosses(sa[0], sa[1], sb[0], sb[1]) {
                        crossings += 1;
                    }
                }
            }
        }
    }
    let mut turns = 0;
    for p in paths {
        let mut at = (0.0, 0.0);
        for s in p {
            if let Seg::Curve(c1, c2, q) = *s
                && (q.0 - at.0).abs() > 0.5
                && (q.1 - at.1).abs() > 0.5
            {
                let (a, b) = ((c1.0 - at.0, c1.1 - at.1), (q.0 - c2.0, q.1 - c2.1));
                turns += if (a.0 * b.1 - a.1 * b.0).abs() < 1e-6 {
                    2
                } else {
                    1
                };
            }
            at = match *s {
                Seg::Move(q) | Seg::Line(q) | Seg::Curve(_, _, q) => q,
            };
        }
    }
    let length: f64 = runs
        .iter()
        .flatten()
        .flat_map(|r| r.windows(2))
        .map(|w| ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt())
        .sum();
    (crossings, turns, length.round() as u64)
}

/// The points along `segs`, each curve cut into short lines.
fn polylines(segs: &[Seg]) -> Vec<Vec<P>> {
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
                        let point = |s: f64, c: f64, d: f64, e: f64| {
                            u * u * u * s
                                + 3.0 * u * u * t * c
                                + 3.0 * u * t * t * d
                                + t * t * t * e
                        };
                        r.push((point(at.0, a.0, b.0, p.0), point(at.1, a.1, b.1, p.1)));
                    }
                }
                at = p;
            }
        }
    }
    runs
}

/// Whether the segments `a`-`b` and `c`-`d` cross, each taken without its end.
fn crosses(a: P, b: P, c: P, d: P) -> bool {
    let den = (b.0 - a.0) * (d.1 - c.1) - (b.1 - a.1) * (d.0 - c.0);
    if den.abs() < 1e-9 {
        return false;
    }
    let t = ((c.0 - a.0) * (d.1 - c.1) - (c.1 - a.1) * (d.0 - c.0)) / den;
    let u = ((c.0 - a.0) * (b.1 - a.1) - (c.1 - a.1) * (b.0 - a.0)) / den;
    (0.0..1.0).contains(&t) && (0.0..1.0).contains(&u)
}

/// The pieces without lines of no length.
fn clean(segs: Vec<Seg>) -> Vec<Seg> {
    let mut out: Vec<Seg> = Vec::with_capacity(segs.len());
    let mut at = (f64::NAN, f64::NAN);
    for s in segs {
        match s {
            Seg::Line(p) if (p.0 - at.0).abs() < 0.01 && (p.1 - at.1).abs() < 0.01 => {}
            Seg::Move(p) => {
                if matches!(out.last(), Some(Seg::Move(_))) {
                    out.pop();
                }
                out.push(s);
                at = p;
            }
            Seg::Line(p) | Seg::Curve(_, _, p) => {
                out.push(s);
                at = p;
            }
        }
    }
    if matches!(out.last(), Some(Seg::Move(_))) {
        out.pop();
    }
    out
}

/// The same path walked from its end.
pub fn reverse(segs: &[Seg]) -> Vec<Seg> {
    // Split into runs that each start with a move.
    let mut runs: Vec<Vec<Seg>> = Vec::new();
    for &s in segs {
        if matches!(s, Seg::Move(_)) || runs.is_empty() {
            runs.push(Vec::new());
        }
        if let Some(run) = runs.last_mut() {
            run.push(s);
        }
    }
    let mut out = Vec::with_capacity(segs.len());
    for run in runs.iter().rev() {
        let points: Vec<P> = run
            .iter()
            .map(|s| match *s {
                Seg::Move(p) | Seg::Line(p) | Seg::Curve(_, _, p) => p,
            })
            .collect();
        let last = points.len() - 1;
        out.push(Seg::Move(points[last]));
        for k in (1..=last).rev() {
            match run[k] {
                Seg::Curve(c1, c2, _) => out.push(Seg::Curve(c2, c1, points[k - 1])),
                _ => out.push(Seg::Line(points[k - 1])),
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spacing() -> Spacing {
        Spacing {
            node: 30.0,
            rank: 40.0,
            edge: 10.0,
            pad: 10.0,
            outside: 16.0,
            turn: 10.0,
        }
    }

    fn node() -> NodeIn {
        NodeIn {
            lo: 20.0,
            hi: 20.0,
            along: 20.0,
            outline: Outline::Rect,
            boxy: true,
        }
    }

    fn edge(from: usize, to: usize) -> EdgeIn {
        EdgeIn {
            from,
            to,
            label: None,
            heads: (0.0, 0.0),
            look: 0,
        }
    }

    #[test]
    fn a_chain_runs_straight_down() {
        let input = Input {
            under_parents: true,
            nodes: vec![node(), node(), node()],
            edges: vec![edge(0, 1), edge(1, 2)],
            groups: Vec::new(),
            title_side: TitleSide::AlongLow,
            spacing: spacing(),
        };
        let out = layout(&input).unwrap();
        assert!(out.nodes[0].1 < out.nodes[1].1 && out.nodes[1].1 < out.nodes[2].1);
        assert!(
            (out.nodes[0].0 - out.nodes[2].0).abs() < 0.5,
            "{:?}",
            out.nodes
        );
    }

    #[test]
    fn nodes_of_a_rank_do_not_overlap() {
        let input = Input {
            under_parents: true,
            nodes: (0..4).map(|_| node()).collect(),
            edges: vec![edge(0, 1), edge(0, 2), edge(0, 3)],
            groups: Vec::new(),
            title_side: TitleSide::AlongLow,
            spacing: spacing(),
        };
        let out = layout(&input).unwrap();
        let mut xs: Vec<f64> = out.nodes[1..].iter().map(|p| p.0).collect();
        xs.sort_by(f64::total_cmp);
        assert!(xs.windows(2).all(|w| w[1] - w[0] >= 70.0 - 1e-6), "{xs:?}");
        let mid = (xs[0] + xs[2]) / 2.0;
        assert!((out.nodes[0].0 - mid).abs() < 1.0, "{:?}", out.nodes);
    }

    #[test]
    fn a_back_edge_goes_around_and_keeps_its_direction() {
        let input = Input {
            under_parents: true,
            nodes: vec![node(), node(), node()],
            edges: vec![edge(0, 1), edge(1, 2), edge(2, 0)],
            groups: Vec::new(),
            title_side: TitleSide::AlongLow,
            spacing: spacing(),
        };
        let out = layout(&input).unwrap();
        let back = &out.edges[2];
        // It leaves the last node and reaches the first, around the middle one.
        let Seg::Move(first) = back[0] else { panic!() };
        assert!(
            (first.1 - out.nodes[2].1).abs() <= out.along[2] / 2.0 + 1e-6,
            "{back:?}"
        );
        let last = match back.last() {
            Some(Seg::Line(p) | Seg::Curve(_, _, p)) => *p,
            _ => panic!(),
        };
        assert!(
            (last.1 - out.nodes[0].1).abs() <= out.along[0] / 2.0 + 1e-6,
            "{back:?}"
        );
        // The bend runs beside the middle node, not through it.
        let beside = back.iter().any(|s| match s {
            Seg::Line(p) => {
                (p.1 - out.nodes[1].1).abs() < 30.0 && (p.0 - out.nodes[1].0).abs() > 20.0
            }
            _ => false,
        });
        assert!(beside, "{back:?}");
    }

    #[test]
    fn a_group_holds_its_nodes_and_keeps_others_out() {
        let input = Input {
            under_parents: true,
            nodes: (0..4).map(|_| node()).collect(),
            edges: vec![edge(0, 1), edge(0, 2), edge(1, 3), edge(2, 3)],
            groups: vec![GroupIn {
                parent: None,
                nodes: vec![1],
                title: (40.0, 14.0),
            }],
            title_side: TitleSide::AlongLow,
            spacing: spacing(),
        };
        let out = layout(&input).unwrap();
        let b = out.groups[0].unwrap();
        let inside = |p: P, lo: f64, hi: f64| {
            p.0 - lo >= b[0] && p.0 + hi <= b[2] && p.1 > b[1] && p.1 < b[3]
        };
        assert!(inside(out.nodes[1], 20.0, 20.0), "{b:?} {:?}", out.nodes);
        for v in [0, 2, 3] {
            let p = out.nodes[v];
            let clear = p.0 + 20.0 <= b[0]
                || p.0 - 20.0 >= b[2]
                || p.1 + 10.0 <= b[1]
                || p.1 - 10.0 >= b[3];
            assert!(clear, "node {v} {p:?} in {b:?}");
        }
        assert!(b[2] - b[0] >= 60.0 - 1e-6);
    }

    #[test]
    fn a_crossing_free_order_is_found() {
        // 2 rows of 3: a-x, a-y, b-z, c-y cross in the order of the source, not as a c b.
        let input = Input {
            under_parents: true,
            nodes: (0..6).map(|_| node()).collect(),
            edges: vec![edge(0, 3), edge(1, 5), edge(2, 4), edge(0, 4)],
            groups: Vec::new(),
            title_side: TitleSide::AlongLow,
            spacing: spacing(),
        };
        let out = layout(&input).unwrap();
        let mut crossings = 0;
        let sampled: Vec<Vec<P>> = out
            .edges
            .iter()
            .map(|segs| {
                segs.iter()
                    .map(|s| match *s {
                        Seg::Move(p) | Seg::Line(p) | Seg::Curve(_, _, p) => p,
                    })
                    .collect()
            })
            .collect();
        for (i, a) in sampled.iter().enumerate() {
            for b in &sampled[i + 1..] {
                for sa in a.windows(2) {
                    for sb in b.windows(2) {
                        let (p, q, r, t) = (sa[0], sa[1], sb[0], sb[1]);
                        let den = (q.0 - p.0) * (t.1 - r.1) - (q.1 - p.1) * (t.0 - r.0);
                        if den.abs() > 1e-9 {
                            let u = ((r.0 - p.0) * (t.1 - r.1) - (r.1 - p.1) * (t.0 - r.0)) / den;
                            let w = ((r.0 - p.0) * (q.1 - p.1) - (r.1 - p.1) * (q.0 - p.0)) / den;
                            if u > 0.0 && u < 1.0 && w > 0.0 && w < 1.0 {
                                crossings += 1;
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(crossings, 0, "{:?}", out.nodes);
    }

    #[test]
    fn an_edge_between_2_nodes_is_straight() {
        let input = Input {
            under_parents: true,
            nodes: vec![
                node(),
                NodeIn {
                    lo: 35.0,
                    hi: 35.0,
                    along: 20.0,
                    outline: Outline::Rect,
                    boxy: true,
                },
            ],
            edges: vec![edge(0, 1)],
            groups: Vec::new(),
            title_side: TitleSide::AlongLow,
            spacing: spacing(),
        };
        let out = layout(&input).unwrap();
        assert!(
            out.edges[0].iter().all(|s| !matches!(s, Seg::Curve(..))),
            "{:?}",
            out.edges[0]
        );
    }

    fn first_point(segs: &[Seg]) -> P {
        match segs[0] {
            Seg::Move(p) | Seg::Line(p) | Seg::Curve(_, _, p) => p,
        }
    }

    #[test]
    fn edges_sharing_a_side_are_spread_in_order() {
        let input = Input {
            under_parents: true,
            nodes: vec![
                NodeIn {
                    lo: 60.0,
                    hi: 60.0,
                    along: 20.0,
                    outline: Outline::Rect,
                    boxy: true,
                },
                node(),
                node(),
            ],
            edges: vec![edge(0, 1), edge(0, 2)],
            groups: Vec::new(),
            title_side: TitleSide::AlongLow,
            spacing: spacing(),
        };
        let out = layout(&input).unwrap();
        let (a, b) = (first_point(&out.edges[0]), first_point(&out.edges[1]));
        assert!((a.0 - b.0).abs() > 10.0, "{a:?} {b:?}");
        assert_eq!(a.0 < b.0, out.nodes[1].0 < out.nodes[2].0);
    }

    #[test]
    fn ports_on_a_wide_side_keep_edges_straight() {
        // A fork of 2 or 3 children meeting again in a wide target: each arrives straight
        // under its child, the ports in order and at least 1 gap apart.
        for k in [2usize, 3] {
            let mut nodes = vec![node()];
            nodes.extend((0..k).map(|_| NodeIn {
                lo: 5.0,
                hi: 5.0,
                along: 20.0,
                outline: Outline::Rect,
                boxy: true,
            }));
            nodes.push(NodeIn {
                lo: 120.0,
                hi: 120.0,
                along: 20.0,
                outline: Outline::Rect,
                boxy: true,
            });
            let t = k + 1;
            let mut edges: Vec<EdgeIn> = (1..=k).map(|c| edge(0, c)).collect();
            edges.extend((1..=k).map(|c| edge(c, t)));
            let input = Input {
                under_parents: true,
                nodes,
                edges,
                groups: Vec::new(),
                title_side: TitleSide::AlongLow,
                spacing: spacing(),
            };
            let out = layout(&input).unwrap();
            let mut at = Vec::new();
            for e in &out.edges[k..] {
                let first = first_point(e);
                let last = match e.last() {
                    Some(Seg::Line(p) | Seg::Curve(_, _, p)) => *p,
                    _ => (f64::NAN, f64::NAN),
                };
                assert!((first.0 - last.0).abs() < 1e-6, "{k} edges: {e:?}");
                at.push(last.0);
            }
            at.sort_by(f64::total_cmp);
            for w in at.windows(2) {
                assert!(w[1] - w[0] >= MIN_GAP - 1e-6, "{k} edges: {at:?}");
            }
        }
    }

    #[test]
    fn an_edge_alone_on_its_side_uses_the_middle() {
        let input = Input {
            under_parents: true,
            nodes: vec![node(), node()],
            edges: vec![edge(0, 1)],
            groups: Vec::new(),
            title_side: TitleSide::AlongLow,
            spacing: spacing(),
        };
        let out = layout(&input).unwrap();
        assert!((first_point(&out.edges[0]).0 - out.nodes[0].0).abs() < 1e-6);
    }

    #[test]
    fn a_back_edge_leaves_by_a_side() {
        // The wide middle node keeps the way back off the column of the chain.
        let wide = NodeIn {
            lo: 60.0,
            hi: 60.0,
            along: 20.0,
            outline: Outline::Rect,
            boxy: true,
        };
        let input = Input {
            under_parents: true,
            nodes: vec![node(), wide, node()],
            edges: vec![edge(0, 1), edge(1, 2), edge(2, 0)],
            groups: Vec::new(),
            title_side: TitleSide::AlongLow,
            spacing: spacing(),
        };
        let out = layout(&input).unwrap();
        let back = &out.edges[2];
        // It goes around the wide middle node: a turn at each end, and a short step where it
        // meets the even ports of the last node.
        let (_, turns, _) = route_cost(std::slice::from_ref(back));
        assert!(turns <= 4, "{back:?}");
        let (c, half) = (out.nodes[1], 60.0);
        for run in polylines(back) {
            for p in run {
                assert!(
                    (p.0 - c.0).abs() >= half - 1e-6 || (p.1 - c.1).abs() >= 10.0 - 1e-6,
                    "{back:?}"
                );
            }
        }
    }

    #[test]
    fn ports_sit_on_the_outline_of_a_diamond() {
        let diamond = NodeIn {
            lo: 50.0,
            hi: 50.0,
            along: 40.0,
            outline: Outline::Diamond,
            boxy: true,
        };
        let input = Input {
            under_parents: true,
            nodes: vec![diamond, node(), node(), node()],
            edges: vec![edge(0, 1), edge(0, 2), edge(0, 3)],
            groups: Vec::new(),
            title_side: TitleSide::AlongLow,
            spacing: spacing(),
        };
        let out = layout(&input).unwrap();
        let c = out.nodes[0];
        for segs in &out.edges {
            let p = first_point(segs);
            let on = (p.0 - c.0).abs() / 50.0 + (p.1 - c.1).abs() / 20.0;
            let side = (p.1 - c.1).abs() < 1e-6 && ((p.0 - c.0).abs() - 50.0).abs() < 1e-6;
            assert!((on - 1.0).abs() < 1e-6 || side, "{p:?} from {c:?}");
        }
    }

    #[test]
    fn a_label_cuts_its_edge() {
        let input = Input {
            under_parents: true,
            nodes: vec![node(), node()],
            edges: vec![EdgeIn {
                from: 0,
                to: 1,
                label: Some((30.0, 14.0)),
                heads: (0.0, 0.0),
                look: 0,
            }],
            groups: Vec::new(),
            title_side: TitleSide::AlongLow,
            spacing: spacing(),
        };
        let out = layout(&input).unwrap();
        let label = out.labels[0].unwrap();
        assert!(label.1 > out.nodes[0].1 && label.1 < out.nodes[1].1);
        assert_eq!(
            out.edges[0]
                .iter()
                .filter(|s| matches!(s, Seg::Move(_)))
                .count(),
            2
        );
    }
}

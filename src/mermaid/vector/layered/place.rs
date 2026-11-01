//! The positions of the items across the ranks: blocks that line up, rounds toward where each
//! wants to sit, under the constraints of the ranks.

use super::*;

impl Layout<'_> {
    /// Improves the current order by sweeps, the first one downward or upward, each followed by
    /// exchanges of neighbours, and keeps the order with the fewest crossings. Returns its count.
    /// Asks each side branch to stand next to its parent, at the room between them.
    pub(super) fn tie(&self, desired: &mut [f64], weight: &mut [f64], x: &[f64], block: &[usize]) {
        for &(u, v, _, after) in &self.flat {
            let sep = self.separation(u, v);
            let b = block[v];
            desired[b] = x[block[u]] + if after { sep } else { -sep };
            weight[b] = weight[b].max(TIE);
        }
    }

    /// Bends that the positions `x` leave: links whose ends do not face each other, a node
    /// taking up the offset across its side.
    pub(super) fn bends(&self, x: &[f64]) -> usize {
        // A main path that leaves a fork off its middle counts 1 more.
        let mut count = 0;
        for list in &self.spine {
            let (a, b) = (list[0], list[1]);
            let shared = self.down[a]
                .iter()
                .filter(|&&j| !matches!(self.items[j].kind, Kind::Border(..)))
                .count()
                > 1;
            if shared && self.offset(a, b).abs() > 0.5 {
                count += 1;
            }
        }
        for (a, links) in self.down.iter().enumerate() {
            if matches!(self.items[a].kind, Kind::Border(..)) {
                continue;
            }
            for &b in links {
                if (x[b] - x[a] - self.want(b, a)).abs() <= 0.5 {
                    // A fork turns once at its vertex.
                    if self.fork_offset(a, b).is_some_and(|o| o != 0.0) {
                        count += 1;
                    }
                    continue;
                }
                // 1 turn when an end can leave or reach a side vertex, else a step of 2.
                let beside = |i: usize, j: usize| match self.items[i].kind {
                    Kind::Node(v) => (x[j] - x[i]).abs() >= self.input.nodes[v].lo + MIN_CORNER,
                    _ => false,
                };
                count += if beside(a, b) || beside(b, a) { 1 } else { 2 };
            }
        }
        count
    }

    /// Positions across the ranks.
    /// Positions across the ranks. With `align`, nodes line up with their middle neighbours and
    /// an edge lines up its bends only where that does not widen the layout; without, the bends
    /// of each edge line up wherever they can. With `top_down`, a node then sits under its parent,
    /// siblings around their parent's axis and a node with several parents at their median.
    pub(super) fn across(&self, align: bool, top_down: bool) -> Vec<f64> {
        let count = self.items.len();
        // Items that share one position: the borders of a group, the bends of an edge.
        let mut block: Vec<usize> = (0..count).collect();
        let mut seps: BTreeMap<(usize, usize), f64> = BTreeMap::new();
        for rank in &self.order {
            for w in rank.windows(2) {
                let s = self.separation(w[0], w[1]);
                let e = seps.entry((w[0], w[1])).or_insert(s);
                *e = e.max(s);
            }
        }
        let mut extra: Vec<(usize, usize, f64)> = Vec::new();
        if self.input.title_side != TitleSide::AcrossLow {
            for (g, group) in self.input.groups.iter().enumerate() {
                let l = self.border(g, false);
                let r = self.border(g, true);
                if let (Some(l), Some(r)) = (l, r) {
                    extra.push((l, r, group.title.0 + 2.0 * self.input.spacing.pad));
                }
            }
        }
        let mut chains: Vec<Vec<usize>> = Vec::new();
        for g in 0..self.input.groups.len() {
            for right in [false, true] {
                let c: Vec<usize> = (0..count)
                    .filter(|&i| self.items[i].kind == Kind::Border(g, right))
                    .collect();
                chains.push(c);
            }
        }
        for e in 0..self.input.edges.len() {
            let c: Vec<usize> = (0..count)
                .filter(|&i| self.items[i].kind == Kind::Dummy(e))
                .collect();
            chains.push(c);
        }
        // Every separation, those of the ranks then those of the group titles.
        let pairs: Vec<(usize, usize, f64)> = seps
            .iter()
            .map(|(&(a, b), &s)| (a, b, s))
            .chain(extra.iter().copied())
            .collect();
        let topo = topological(&block, &pairs).unwrap_or_default();
        let borders = 2 * self.input.groups.len();
        let mut width = self.compact_width(&block, &topo, &pairs);
        // Each merge of blocks is tried on the constraints: kept when they stay without a cycle,
        // and for most when the compact width stays close.
        let mut probe = Probe::new(count, pairs.len());
        for (k, chain) in chains.iter().enumerate().filter(|(_, c)| c.len() > 1) {
            let saved = block.clone();
            let root = block[chain[0]];
            for &i in chain {
                let old = block[i];
                for b in block.iter_mut() {
                    if *b == old {
                        *b = root;
                    }
                }
            }
            // The borders of a group always line up; the bends of an edge do when that does not
            // widen the layout.
            match probe.width(self, &block, &pairs) {
                Some(w) => {
                    if !align || k < borders || w <= width + WIDER {
                        width = width.max(w);
                    } else {
                        block = saved;
                    }
                }
                None => block = saved,
            }
        }
        // An item faces its median neighbour above when each is the middle neighbour of the
        // other and nothing lies between them, so that a chain and a link to a single successor
        // run straight. Without a middle neighbour, the item stays between its neighbours.
        let mut pos = vec![0usize; count];
        for rank in &self.order {
            for (p, &i) in rank.iter().enumerate() {
                pos[i] = p;
            }
        }
        let movable = |i: usize| !matches!(self.items[i].kind, Kind::Border(..));
        // With an even count, the edges to the next rank of nodes count alone: an edge to a far
        // rank is not the one that carries on.
        let middle = |links: &[usize]| -> Option<usize> {
            let mut list: Vec<usize> = links.iter().copied().filter(|&u| movable(u)).collect();
            if list.len().is_multiple_of(2) {
                list.retain(|&u| self.items[u].short);
            }
            if list.len().is_multiple_of(2) {
                return None;
            }
            list.sort_by_key(|&u| pos[u]);
            Some(list[list.len() / 2])
        };
        // The main path lines up, edge after edge from the top, wherever no cycle forbids it.
        for list in &self.spine {
            let saved = block.clone();
            let root = block[list[0]];
            for &i in list {
                let old = block[i];
                for b in block.iter_mut() {
                    if *b == old {
                        *b = root;
                    }
                }
            }
            if probe.width(self, &block, &pairs).is_none() {
                block = saved;
            }
        }
        // In every mode, the 2 ends of an edge that is alone on both their sides line up.
        let border = |i: usize| matches!(self.items[i].kind, Kind::Border(..));
        let degree = |links: &[usize]| links.iter().filter(|&&j| !border(j)).count();
        for rank in self.order.iter().skip(1) {
            for &v in rank {
                if border(v) || degree(&self.up[v]) != 1 {
                    continue;
                }
                let Some(&u) = self.up[v].iter().find(|&&j| !border(j)) else {
                    continue;
                };
                if degree(&self.down[u]) != 1 || block[u] == block[v] {
                    continue;
                }
                let saved = block.clone();
                let (old, root) = (block[v], block[u]);
                for b in block.iter_mut() {
                    if *b == old {
                        *b = root;
                    }
                }
                match probe.width(self, &block, &pairs) {
                    Some(w) if w <= width + WIDER => {}
                    _ => block = saved,
                }
            }
        }
        let aligned: &[Vec<usize>] = if align { &self.order[1..] } else { &[] };
        for rank in aligned {
            let mut last: Option<usize> = None;
            for &v in rank {
                if !movable(v) {
                    continue;
                }
                let Some(u) = middle(&self.up[v]) else {
                    continue;
                };
                if middle(&self.down[u]) != Some(v)
                    || block[u] == block[v]
                    || last.is_some_and(|l| pos[u] <= l)
                {
                    continue;
                }
                let saved = block.clone();
                let (old, root) = (block[v], block[u]);
                for b in block.iter_mut() {
                    if *b == old {
                        *b = root;
                    }
                }
                match probe.width(self, &block, &pairs) {
                    Some(w) if w <= width + WIDER => {
                        last = Some(pos[u]);
                    }
                    _ => block = saved,
                }
            }
        }
        let topo = topological(&block, &pairs).unwrap_or_default();
        // Constraints between blocks, the largest of each pair.
        let mut cons: BTreeMap<(usize, usize), f64> = BTreeMap::new();
        for (&(a, b), &s) in &seps {
            let e = cons.entry((block[a], block[b])).or_insert(s);
            *e = e.max(s);
        }
        for &(a, b, s) in &extra {
            let e = cons.entry((block[a], block[b])).or_insert(s);
            *e = e.max(s);
        }
        let cons: Vec<(usize, usize, f64)> =
            cons.into_iter().map(|((a, b), s)| (a, b, s)).collect();
        let blocks: Vec<usize> = topo;
        let mut before: Vec<Vec<(usize, f64)>> = vec![Vec::new(); count];
        for &(a, c, s) in &cons {
            before[c].push((a, s));
        }
        let mut x = vec![0.0f64; count];
        for &b in &blocks {
            for &(a, s) in &before[b] {
                x[b] = x[b].max(x[a] + s);
            }
        }
        let mut members: Vec<Vec<usize>> = vec![Vec::new(); count];
        for (i, &b) in block.iter().enumerate() {
            members[b].push(i);
        }
        let weight_of = |a: usize, b: usize| {
            let dummy = |i: usize| matches!(self.items[i].kind, Kind::Dummy(_));
            match (dummy(a), dummy(b)) {
                (true, true) => 8.0,
                (false, false) => 1.0,
                _ => 2.0,
            }
        };
        // What pulls each block, in the order of its members: a group border hugs what its group
        // holds, an item is pulled by each neighbour of another block, with a weight, toward where
        // it wants to sit from it. The blocks stay the same through the rounds.
        let pulls: Vec<Vec<Pull>> = (0..count)
            .map(|b| {
                let mut list = Vec::new();
                for &i in &members[b] {
                    match self.items[i].kind {
                        Kind::Border(g, right) => list.push(Pull::Hug(g, right)),
                        _ => {
                            for &j in self.up[i].iter().chain(&self.down[i]) {
                                if block[j] != b && !self.loose(i, j) {
                                    list.push(Pull::Link(
                                        block[j],
                                        weight_of(i, j),
                                        self.want(i, j),
                                    ));
                                }
                            }
                        }
                    }
                }
                list
            })
            .collect();
        let mut projector = Projector::new(&cons, count);
        for _ in 0..ROUNDS {
            let mut desired = vec![0.0; count];
            let mut weight = vec![0.0; count];
            for &b in &blocks {
                let (mut sum, mut wsum) = (0.0, 0.0);
                for pull in &pulls[b] {
                    match *pull {
                        Pull::Hug(g, right) => {
                            if let Some(edge) = self.hug(g, right, &x, &block) {
                                sum += edge;
                                wsum += 1.0;
                            }
                        }
                        Pull::Link(j, w, want) => {
                            sum += w * (x[j] + want);
                            wsum += w;
                        }
                    }
                }
                if wsum > 0.0 {
                    desired[b] = (x[b] + sum / wsum) / 2.0;
                    weight[b] = wsum;
                } else {
                    desired[b] = x[b];
                    weight[b] = 0.1;
                }
            }
            self.tie(&mut desired, &mut weight, &x, &block);
            x = projector.project(&blocks, &desired, &weight);
        }
        if top_down {
            for round in 0..TOP_DOWN_ROUNDS {
                let downward = round % 2 == 0;
                let (mut desired, mut weight) =
                    self.hierarchy(&x, &block, &blocks, &members, downward);
                self.tie(&mut desired, &mut weight, &x, &block);
                x = projector.project(&blocks, &desired, &weight);
            }
        }
        let positions: Vec<f64> = (0..count).map(|i| x[block[i]]).collect();
        let positions = self.straighten(positions, &members);
        let low = (0..count)
            .map(|i| positions[i] - self.items[i].lo)
            .fold(f64::INFINITY, f64::min);
        positions.iter().map(|p| p - low).collect()
    }

    /// Where each block wants to be: downward, under its parent among its siblings, or at the
    /// median of its parents; upward, over the middle of the children it is the only parent of.
    pub(super) fn hierarchy(
        &self,
        x: &[f64],
        block: &[usize],
        blocks: &[usize],
        members: &[Vec<usize>],
        downward: bool,
    ) -> (Vec<f64>, Vec<f64>) {
        let count = self.items.len();
        let border = |i: usize| matches!(self.items[i].kind, Kind::Border(..));
        let mut pos = vec![0usize; count];
        for rank in &self.order {
            for (p, &i) in rank.iter().enumerate() {
                pos[i] = p;
            }
        }
        let mut desired = vec![0.0; count];
        let mut weight = vec![0.0; count];
        for &b in blocks {
            let (mut sum, mut n) = (0.0, 0.0);
            for &i in &members[b] {
                if border(i) {
                    if let Kind::Border(g, right) = self.items[i].kind
                        && let Some(edge) = self.hug(g, right, x, block)
                    {
                        sum += edge;
                        n += 1.0;
                    }
                    continue;
                }
                let near = if downward { &self.up[i] } else { &self.down[i] };
                let others: Vec<usize> = near
                    .iter()
                    .copied()
                    .filter(|&j| block[j] != b && !border(j))
                    .filter(|&j| !self.loose(i, j))
                    .collect();
                // Each neighbour asks for the position that makes their edge straight between
                // its 2 ports.
                let facing = |j: usize| x[block[j]] + self.want(i, j);
                if downward && others.len() == 1 {
                    // Under the parent at its port, or past its vertex on a fork. Children too
                    // wide for the ports of the side pack around the parent's axis.
                    let p = others[0];
                    let mut sibs: Vec<usize> = self.down[p]
                        .iter()
                        .copied()
                        .filter(|&j| !border(j))
                        .collect();
                    sibs.sort_by_key(|&j| pos[j]);
                    let mut packed = Vec::with_capacity(sibs.len());
                    let mut at = 0.0;
                    for (k, &j) in sibs.iter().enumerate() {
                        if k > 0 {
                            at += self.separation(sibs[k - 1], j);
                        }
                        packed.push(at);
                    }
                    let middle = (packed[0] + packed[packed.len() - 1]) / 2.0;
                    let k = sibs.iter().position(|&j| j == i).unwrap_or(0);
                    let pack = packed[k] - middle;
                    let want = self.want(i, p);
                    let offset =
                        if self.fork_offset(p, i).is_some() || pack.abs() <= want.abs() + 0.5 {
                            want
                        } else {
                            pack
                        };
                    sum += x[block[p]] + offset;
                    n += 1.0;
                } else if downward {
                    // At the median of several parents.
                    if !others.is_empty() {
                        let mut xs: Vec<f64> = others.iter().map(|&j| facing(j)).collect();
                        xs.sort_by(f64::total_cmp);
                        let m = xs.len();
                        sum += (xs[(m - 1) / 2] + xs[m / 2]) / 2.0;
                        n += 1.0;
                    }
                } else {
                    // Over its children: over the only one, or over its port on a merge.
                    if !others.is_empty() {
                        sum += others.iter().map(|&j| facing(j)).sum::<f64>() / others.len() as f64;
                        n += 1.0;
                    }
                }
            }
            if n > 0.0 {
                desired[b] = sum / n;
                weight[b] = n;
            } else {
                desired[b] = x[b];
                weight[b] = 0.1;
            }
        }
        (desired, weight)
    }

    /// The offset of child `c` from its parent `p` when `p` forks to 2 or 3 children: the outer
    /// ones past the side vertices by the radius of the turn, the third one under it. `None` for
    /// another link.
    pub(super) fn fork_offset(&self, p: usize, c: usize) -> Option<f64> {
        let Kind::Node(v) = self.items[p].kind else {
            return None;
        };
        let border = |i: usize| matches!(self.items[i].kind, Kind::Border(..));
        if !self.down[p].contains(&c) {
            return None;
        }
        let mut sibs: Vec<usize> = self.down[p]
            .iter()
            .copied()
            .filter(|&j| !border(j))
            .collect();
        if !(2..=3).contains(&sibs.len()) {
            return None;
        }
        let r = self.items[c].rank;
        sibs.sort_by_key(|&l| self.order[r].iter().position(|&o| o == l).unwrap_or(0));
        let k = sibs.iter().position(|&j| j == c)?;
        let reach = self.input.nodes[v].lo + self.input.spacing.turn;
        Some(if k == 0 {
            -reach
        } else if k == sibs.len() - 1 {
            reach
        } else {
            0.0
        })
    }

    /// Whether the link between `i` and `j` leaves or reaches a node by a side across the flow and
    /// asks no position: a fork keeps its pull.
    pub(super) fn loose(&self, i: usize, j: usize) -> bool {
        self.lateral.borrow().contains(&(i, j))
            && self.fork_offset(i, j).is_none()
            && self.fork_offset(j, i).is_none()
    }

    /// How far across from its neighbour `j` item `i` wants to sit: past a side vertex for the
    /// outer child of a fork, else so that their edge runs straight between its 2 ports.
    pub(super) fn want(&self, i: usize, j: usize) -> f64 {
        if let Some(o) = self.fork_offset(j, i) {
            return o;
        }
        if let Some(o) = self.fork_offset(i, j) {
            return -o;
        }
        self.offset(j, i) - self.offset(i, j)
    }

    /// Where the edge from item `i` to its neighbour `j` attaches across, from the centre of `i`:
    /// n edges on a side sit at (k + 1) / (n + 1) of it, in the order of their neighbours.
    pub(super) fn offset(&self, i: usize, j: usize) -> f64 {
        let Kind::Node(v) = self.items[i].kind else {
            return 0.0;
        };
        let links = if self.down[i].contains(&j) {
            &self.down[i]
        } else {
            &self.up[i]
        };
        let lateral = self.lateral.borrow();
        let mut list: Vec<usize> = links
            .iter()
            .copied()
            .filter(|&l| !matches!(self.items[l].kind, Kind::Border(..)))
            .filter(|&l| l == j || !lateral.contains(&(i, l)))
            .collect();
        let r = self.items[j].rank;
        list.sort_by_key(|&l| self.order[r].iter().position(|&o| o == l).unwrap_or(0));
        let n = list.len();
        let k = list.iter().position(|&l| l == j).unwrap_or(0);
        let lo = self.input.nodes[v].lo;
        -lo + 2.0 * lo * (k + 1) as f64 / (n + 1) as f64
    }

    /// How far across from the centre of item `i` an edge to its neighbour may attach: 0 for a
    /// node with that edge alone on its side, which takes the middle.
    pub(super) fn end_slack(&self, i: usize, downward: bool) -> f64 {
        let Kind::Node(v) = self.items[i].kind else {
            return 0.0;
        };
        let links = if downward { &self.down[i] } else { &self.up[i] };
        let shared = links
            .iter()
            .filter(|&&j| !matches!(self.items[j].kind, Kind::Border(..)))
            .count()
            > 1;
        if shared {
            side_reach(&self.input.nodes[v])
        } else {
            0.0
        }
    }

    /// Moves each block that is not a group border to where more of its links run straight,
    /// within the room its ranks leave: the order stays, so no crossing comes.
    pub(super) fn straighten(&self, mut x: Vec<f64>, members: &[Vec<usize>]) -> Vec<f64> {
        let count = self.items.len();
        let mut place = vec![(0usize, 0usize); count];
        for (r, rank) in self.order.iter().enumerate() {
            for (p, &i) in rank.iter().enumerate() {
                place[i] = (r, p);
            }
        }
        for _ in 0..6 {
            let mut moved = false;
            for list in members.iter().filter(|m| !m.is_empty()) {
                // A group border stays, and so does a node with several parents, at their median.
                let border = |i: usize| matches!(self.items[i].kind, Kind::Border(..));
                let merge = |i: usize| {
                    matches!(self.items[i].kind, Kind::Node(_))
                        && self.up[i].iter().filter(|&&j| !border(j)).count() > 1
                };
                if list.iter().any(|&i| border(i) || merge(i)) {
                    continue;
                }
                let (mut lo, mut hi) = (f64::NEG_INFINITY, f64::INFINITY);
                let mut targets = Vec::new();
                for &i in list {
                    for &j in self.up[i].iter().chain(&self.down[i]) {
                        if list.contains(&j)
                            || matches!(self.items[j].kind, Kind::Border(..))
                            || self.loose(i, j)
                        {
                            continue;
                        }
                        targets.push(x[j] + self.want(i, j));
                    }
                    let (r, p) = place[i];
                    let rank = &self.order[r];
                    if p > 0 {
                        let l = rank[p - 1];
                        lo = lo.max(x[l] + self.separation(l, i));
                    }
                    if p + 1 < rank.len() {
                        let n = rank[p + 1];
                        hi = hi.min(x[n] - self.separation(i, n));
                    }
                }
                let at = x[list[0]];
                let straight = |t: f64| targets.iter().filter(|&&u| (u - t).abs() < 0.5).count();
                let now = straight(at);
                let best = targets
                    .iter()
                    .copied()
                    .filter(|&t| t >= lo - 1e-9 && t <= hi + 1e-9)
                    .map(|t| (straight(t), -(t - at).abs(), t))
                    .max_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
                if let Some((more, _, to)) = best
                    && more > now
                {
                    for &i in list {
                        x[i] = to;
                    }
                    moved = true;
                }
            }
            if !moved {
                break;
            }
        }
        x
    }

    /// Width of the layout with every block pushed to the left as far as the constraints let it.
    pub(super) fn compact_width(
        &self,
        block: &[usize],
        topo: &[usize],
        pairs: &[(usize, usize, f64)],
    ) -> f64 {
        let count = block.len();
        let (start, from) = incoming(block, pairs);
        let mut x = vec![0.0f64; count];
        for &b in topo {
            for &(a, s) in &from[start[b]..start[b + 1]] {
                x[b] = x[b].max(x[a] + s);
            }
        }
        let hi = (0..count)
            .map(|i| x[block[i]] + self.items[i].hi)
            .fold(f64::NEG_INFINITY, f64::max);
        let lo = (0..count)
            .map(|i| x[block[i]] - self.items[i].lo)
            .fold(f64::INFINITY, f64::min);
        hi - lo
    }

    /// Where the border `right` of group `g` would touch what the group holds.
    pub(super) fn hug(&self, g: usize, right: bool, x: &[f64], block: &[usize]) -> Option<f64> {
        let pad = self.input.spacing.pad;
        let inside = (0..self.items.len()).filter(|&i| {
            self.chain_of(i).contains(&g)
                && self.items[i].kind != Kind::Border(g, !right)
                && self.items[i].kind != Kind::Border(g, right)
        });
        if right {
            inside
                .map(|i| x[block[i]] + self.items[i].hi + pad)
                .reduce(f64::max)
        } else {
            inside
                .map(|i| x[block[i]] - self.items[i].lo - pad)
                .reduce(f64::min)
        }
    }
}

/// What pulls a block across in a round of the placement.
pub(super) enum Pull {
    /// A group border, toward what its group holds: the group and the side.
    Hug(usize, bool),
    /// A link to another block: that block, the weight, and the offset wanted from it.
    Link(usize, f64, f64),
}

/// Buffers to try merges of blocks, kept from one try to the next.
pub(super) struct Probe {
    /// The blocks each block must precede, with the separations, as slices of one list.
    start: Vec<usize>,
    fill: Vec<usize>,
    next: Vec<(usize, f64)>,
    indegree: Vec<usize>,
    ready: Vec<usize>,
    /// The position of each block pushed to the left.
    x: Vec<f64>,
}

impl Probe {
    pub(super) fn new(count: usize, pairs: usize) -> Probe {
        Probe {
            start: vec![0; count + 1],
            fill: vec![0; count + 1],
            next: vec![(0, 0.0); pairs],
            indegree: vec![0; count],
            ready: Vec::with_capacity(count),
            x: vec![0.0; count],
        }
    }

    /// The compact width of `lay` with the items in `block`, as `Layout::compact_width` finds
    /// it, or `None` when the constraints make a cycle.
    pub(super) fn width(
        &mut self,
        lay: &Layout,
        block: &[usize],
        pairs: &[(usize, usize, f64)],
    ) -> Option<f64> {
        let n = block.len();
        self.start.fill(0);
        self.indegree.fill(0);
        for &(a, b, _) in pairs {
            let (a, b) = (block[a], block[b]);
            if a == b {
                return None;
            }
            self.start[a + 1] += 1;
            self.indegree[b] += 1;
        }
        for k in 0..n {
            self.start[k + 1] += self.start[k];
        }
        self.fill.copy_from_slice(&self.start);
        for &(a, b, s) in pairs {
            let a = block[a];
            self.next[self.fill[a]] = (block[b], s);
            self.fill[a] += 1;
        }
        // Each block takes its place once all the blocks before it have theirs.
        self.x.fill(0.0);
        self.ready.clear();
        let mut roots = 0;
        for (b, &root) in block.iter().enumerate() {
            if root == b {
                roots += 1;
                if self.indegree[b] == 0 {
                    self.ready.push(b);
                }
            }
        }
        let mut placed = 0;
        while let Some(b) = self.ready.pop() {
            placed += 1;
            for k in self.start[b]..self.start[b + 1] {
                let (c, s) = self.next[k];
                self.x[c] = self.x[c].max(self.x[b] + s);
                self.indegree[c] -= 1;
                if self.indegree[c] == 0 {
                    self.ready.push(c);
                }
            }
        }
        if placed != roots {
            return None;
        }
        let hi = (0..n)
            .map(|i| self.x[block[i]] + lay.items[i].hi)
            .fold(f64::NEG_INFINITY, f64::max);
        let lo = (0..n)
            .map(|i| self.x[block[i]] - lay.items[i].lo)
            .fold(f64::INFINITY, f64::min);
        Some(hi - lo)
    }
}

/// The blocks in an order where each constraint goes forward, `None` when they make a cycle.
pub(super) fn topological(block: &[usize], pairs: &[(usize, usize, f64)]) -> Option<Vec<usize>> {
    let n = block.len();
    // The blocks each block must precede, in the order of the pairs, as slices of one list.
    let mut start = vec![0usize; n + 1];
    let mut indegree = vec![0usize; n];
    for &(a, b, _) in pairs {
        let (a, b) = (block[a], block[b]);
        if a == b {
            return None;
        }
        start[a + 1] += 1;
        indegree[b] += 1;
    }
    for k in 0..n {
        start[k + 1] += start[k];
    }
    let mut fill = start.clone();
    let mut next = vec![0usize; pairs.len()];
    for &(a, b, _) in pairs {
        let a = block[a];
        next[fill[a]] = block[b];
        fill[a] += 1;
    }
    let roots: Vec<usize> = (0..n).filter(|&b| block[b] == b).collect();
    let mut ready: Vec<usize> = roots
        .iter()
        .rev()
        .copied()
        .filter(|&b| indegree[b] == 0)
        .collect();
    let mut order = Vec::with_capacity(roots.len());
    while let Some(b) = ready.pop() {
        order.push(b);
        for &c in next[start[b]..start[b + 1]].iter().rev() {
            indegree[c] -= 1;
            if indegree[c] == 0 {
                ready.push(c);
            }
        }
    }
    (order.len() == roots.len()).then_some(order)
}

/// The blocks that each block must follow, with their separations, in the order of the pairs, as
/// slices of one list: those of block `b` lie from `start[b]` to `start[b + 1]`.
pub(super) fn incoming(
    block: &[usize],
    pairs: &[(usize, usize, f64)],
) -> (Vec<usize>, Vec<(usize, f64)>) {
    let n = block.len();
    let mut start = vec![0usize; n + 1];
    for &(_, b, _) in pairs {
        start[block[b] + 1] += 1;
    }
    for k in 0..n {
        start[k + 1] += start[k];
    }
    let mut fill = start.clone();
    let mut from = vec![(0usize, 0.0f64); pairs.len()];
    for &(a, b, s) in pairs {
        let b = block[b];
        from[fill[b]] = (block[a], s);
        fill[b] += 1;
    }
    (start, from)
}

/// Places blocks as close as the constraints let them to where they want to be, with buffers
/// kept from one placement to the next over the same constraints.
pub(super) struct Projector {
    /// The constraints that end at each block, `(a, s)` for `x[b] - x[a] >= s`, in their order.
    incoming: Vec<Vec<(usize, f64)>>,
    /// The cluster of each block, `usize::MAX` before it is placed, and its offset in it.
    cluster: Vec<usize>,
    offset: Vec<f64>,
    /// The blocks of each cluster.
    vars: Vec<Vec<usize>>,
    /// The position of each cluster, and its sums of weights and of weighted wishes.
    posn: Vec<f64>,
    wsum: Vec<f64>,
    wdsum: Vec<f64>,
}

impl Projector {
    /// A projector for the constraints `cons` between `count` blocks.
    pub(super) fn new(cons: &[(usize, usize, f64)], count: usize) -> Projector {
        let mut incoming: Vec<Vec<(usize, f64)>> = vec![Vec::new(); count];
        for &(a, b, s) in cons {
            incoming[b].push((a, s));
        }
        Projector {
            incoming,
            cluster: vec![usize::MAX; count],
            offset: vec![0.0; count],
            vars: vec![Vec::new(); count],
            posn: vec![0.0; count],
            wsum: vec![0.0; count],
            wdsum: vec![0.0; count],
        }
    }

    /// Positions of the blocks closest to `desired`, weighed by `weight`, that keep every
    /// constraint. Blocks come in an order where each constraint goes forward.
    pub(super) fn project(
        &mut self,
        blocks: &[usize],
        desired: &[f64],
        weight: &[f64],
    ) -> Vec<f64> {
        let count = self.cluster.len();
        self.cluster.fill(usize::MAX);
        self.offset.fill(0.0);
        self.posn.fill(0.0);
        self.wsum.fill(0.0);
        self.wdsum.fill(0.0);
        for v in &mut self.vars {
            v.clear();
        }
        let Projector {
            incoming,
            cluster,
            offset,
            vars,
            posn,
            wsum,
            wdsum,
        } = self;
        // Each variable sits in a cluster at an offset from the cluster position.
        for &v in blocks {
            cluster[v] = v;
            vars[v].push(v);
            wsum[v] = weight[v];
            wdsum[v] = weight[v] * desired[v];
            posn[v] = desired[v];
            let mut c = v;
            loop {
                let mut worst: Option<(usize, usize, f64, f64)> = None;
                for &m in &vars[c] {
                    for &(a, s) in &incoming[m] {
                        let ca = cluster[a];
                        if ca == c || ca == usize::MAX {
                            continue;
                        }
                        let violation = posn[ca] + offset[a] + s - (posn[c] + offset[m]);
                        if violation > 1e-9 && worst.is_none_or(|w| violation > w.3) {
                            worst = Some((a, m, s, violation));
                        }
                    }
                }
                let Some((a, m, s, _)) = worst else { break };
                let target = cluster[a];
                let shift = offset[a] + s - offset[m];
                let moved = std::mem::take(&mut vars[c]);
                for &u in &moved {
                    offset[u] += shift;
                    cluster[u] = target;
                    wdsum[target] += weight[u] * (desired[u] - offset[u]);
                    wsum[target] += weight[u];
                }
                vars[target].extend(moved);
                posn[target] = wdsum[target] / wsum[target];
                c = target;
            }
        }
        (0..count)
            .map(|v| {
                if cluster[v] == usize::MAX {
                    0.0
                } else {
                    posn[cluster[v]] + offset[v]
                }
            })
            .collect()
    }
}

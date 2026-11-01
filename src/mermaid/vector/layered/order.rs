//! The order of the items in each rank: first walks, sweeps, exchanges and balanced forks.

use super::*;

impl Layout<'_> {
    /// Mean position of each group's items over the ranks, as a share of the rank.
    pub(super) fn group_keys(&self) -> Vec<f64> {
        let mut sum = vec![0.0; self.input.groups.len()];
        let mut count = vec![0usize; self.input.groups.len()];
        for rank in &self.order {
            let len = rank.len().max(1) as f64;
            for (p, &i) in rank.iter().enumerate() {
                for &g in self.chain_of(i) {
                    sum[g] += p as f64 / len;
                    count[g] += 1;
                }
            }
        }
        sum.iter()
            .zip(&count)
            .map(|(&s, &c)| if c == 0 { 0.0 } else { s / c as f64 })
            .collect()
    }

    /// Orders `items`, the part of a rank inside `level`, by `key`: each group stays together
    /// between its borders, groups side by side keep the order of `gkey`, and an item without a
    /// key keeps its place.
    pub(super) fn arrange(
        &self,
        level: Option<usize>,
        items: &[usize],
        key: &dyn Fn(usize) -> Option<f64>,
        gkey: &[f64],
    ) -> Vec<usize> {
        let depth = level.map_or(0, |g| self.chains[g].len());
        let mut borders = (None, None);
        // A block is an item at this level, or a group inside it with its items.
        let mut blocks: Vec<(Option<usize>, Vec<usize>)> = Vec::new();
        for &i in items {
            if let Kind::Border(g, right) = self.items[i].kind
                && Some(g) == level
            {
                if right {
                    borders.1 = Some(i);
                } else {
                    borders.0 = Some(i);
                }
                continue;
            }
            let chain = self.chain_of(i);
            if chain.len() > depth {
                let child = chain[chain.len() - 1 - depth];
                match blocks.iter_mut().find(|b| b.0 == Some(child)) {
                    Some(block) => block.1.push(i),
                    None => blocks.push((Some(child), vec![i])),
                }
            } else {
                blocks.push((None, vec![i]));
            }
        }
        let keys: Vec<Option<f64>> = blocks
            .iter()
            .map(|(_, members)| {
                let ks: Vec<f64> = members
                    .iter()
                    .filter(|&&i| !matches!(self.items[i].kind, Kind::Border(..)))
                    .filter_map(|&i| key(i))
                    .collect();
                (!ks.is_empty()).then(|| ks.iter().sum::<f64>() / ks.len() as f64)
            })
            .collect();
        let mut sortable: Vec<usize> = (0..blocks.len()).filter(|&b| keys[b].is_some()).collect();
        sortable.sort_by(|&a, &b| {
            keys[a]
                .unwrap_or(0.0)
                .total_cmp(&keys[b].unwrap_or(0.0))
                .then(a.cmp(&b))
        });
        let mut placed: Vec<usize> = Vec::with_capacity(blocks.len());
        let mut fixed = (0..blocks.len()).filter(|&b| keys[b].is_none()).peekable();
        let mut sorted = sortable.into_iter();
        while placed.len() < blocks.len() {
            if fixed.peek().is_some_and(|&b| b <= placed.len()) {
                placed.extend(fixed.next());
            } else if let Some(b) = sorted.next() {
                placed.push(b);
            } else {
                placed.extend(fixed.next());
            }
        }
        // Groups side by side take the same order in every rank.
        let slots: Vec<usize> = (0..placed.len())
            .filter(|&s| blocks[placed[s]].0.is_some())
            .collect();
        let mut groups: Vec<usize> = slots.iter().map(|&s| placed[s]).collect();
        groups.sort_by(|&a, &b| {
            let (ga, gb) = (blocks[a].0.unwrap_or(0), blocks[b].0.unwrap_or(0));
            gkey[ga].total_cmp(&gkey[gb]).then(a.cmp(&b))
        });
        for (&s, &b) in slots.iter().zip(&groups) {
            placed[s] = b;
        }
        let mut out = Vec::with_capacity(items.len());
        out.extend(borders.0);
        for b in placed {
            match blocks[b].0 {
                Some(child) => out.extend(self.arrange(Some(child), &blocks[b].1, key, gkey)),
                None => out.extend(&blocks[b].1),
            }
        }
        out.extend(borders.1);
        out
    }

    /// Puts the main path in the middle of each fork it leaves: the other children that end in
    /// a leaf stand on both sides of it, as many on each, when that adds no crossing.
    pub(super) fn balance(&mut self) {
        let border = |l: &Layout, i: usize| matches!(l.items[i].kind, Kind::Border(..));
        for list in self.spine.clone() {
            if list.len() != 3 {
                continue;
            }
            let (p, d, c) = (list[0], list[1], list[2]);
            let mut sibs: Vec<(usize, usize)> = Vec::new();
            for &s in &self.down[p] {
                if s == d || border(self, s) || self.items[s].group != self.items[d].group {
                    continue;
                }
                let [leaf] = self.down[s][..] else { continue };
                let lone = matches!(self.items[leaf].kind, Kind::Node(_))
                    && self.down[leaf].is_empty()
                    && self.up[leaf].len() == 1
                    && self.items[leaf].rank == self.items[c].rank
                    && self.items[leaf].group == self.items[c].group;
                if lone {
                    sibs.push((s, leaf));
                }
            }
            if sibs.is_empty() {
                continue;
            }
            let (r1, r2) = (self.items[d].rank, self.items[c].rank);
            let at = |l: &Layout, r: usize, i: usize| l.order[r].iter().position(|&j| j == i);
            sibs.sort_by_key(|&(s, _)| at(self, r1, s));
            let half = sibs.len() / 2;
            let mut upper: Vec<usize> = sibs[..half].iter().map(|s| s.0).collect();
            upper.push(d);
            upper.extend(sibs[half..].iter().map(|s| s.0));
            let mut lower: Vec<usize> = sibs[..half].iter().map(|s| s.1).collect();
            lower.push(c);
            lower.extend(sibs[half..].iter().map(|s| s.1));
            let before = (self.order.clone(), self.crossings());
            let mut fits = true;
            for (r, seq) in [(r1, &upper), (r2, &lower)] {
                let mut slots: Vec<usize> = seq.iter().filter_map(|&i| at(self, r, i)).collect();
                slots.sort_unstable();
                if slots.len() != seq.len() || slots[slots.len() - 1] - slots[0] + 1 != slots.len()
                {
                    fits = false;
                    break;
                }
                for (&slot, &i) in slots.iter().zip(seq.iter()) {
                    self.order[r][slot] = i;
                }
            }
            if !fits || self.crossings() > before.1 {
                self.order = before.0;
            }
        }
    }

    pub(super) fn crossings(&self) -> usize {
        let mut pos = vec![0usize; self.items.len()];
        for rank in &self.order {
            for (p, &i) in rank.iter().enumerate() {
                pos[i] = p;
            }
        }
        let mut total = 0;
        for rank in &self.order {
            let mut segs: Vec<(usize, usize)> = Vec::new();
            for &a in rank {
                for &b in &self.down[a] {
                    segs.push((pos[a], pos[b]));
                }
            }
            for (i, s) in segs.iter().enumerate() {
                for t in &segs[i + 1..] {
                    if (s.0 < t.0 && s.1 > t.1) || (s.0 > t.0 && s.1 < t.1) {
                        total += 1;
                    }
                }
            }
        }
        total
    }

    pub(super) fn sweep(&mut self, downward: bool) {
        let n = self.order.len();
        let gkey = self.group_keys();
        let ranks: Vec<usize> = if downward {
            (1..n).collect()
        } else {
            (0..n.saturating_sub(1)).rev().collect()
        };
        for r in ranks {
            let fixed = if downward { r - 1 } else { r + 1 };
            let mut pos = BTreeMap::new();
            for (p, &i) in self.order[fixed].iter().enumerate() {
                pos.insert(i, p as f64);
            }
            let links = if downward { &self.up } else { &self.down };
            let key = |i: usize| {
                let ps: Vec<f64> = links[i]
                    .iter()
                    .filter_map(|j| pos.get(j).copied())
                    .collect();
                (!ps.is_empty()).then(|| ps.iter().sum::<f64>() / ps.len() as f64)
            };
            let current = self.order[r].clone();
            self.order[r] = self.arrange(None, &current, &key, &gkey);
        }
    }

    /// Puts each side branch next to its parent in their rank, after it or before it.
    pub(super) fn pin(&mut self) {
        for &(u, v, _, after) in &self.flat {
            let r = self.items[u].rank;
            let rank = &mut self.order[r];
            rank.retain(|&i| i != v);
            let p = rank.iter().position(|&i| i == u).unwrap_or(0);
            rank.insert(if after { p + 1 } else { p }, v);
        }
    }

    pub(super) fn improve(&mut self, first_down: bool, exchange: bool) -> usize {
        self.pin();
        if exchange {
            self.transpose();
        }
        let mut best = self.order.clone();
        let mut best_count = self.crossings();
        let mut stale = 0;
        for s in 0..SWEEPS {
            if best_count == 0 || stale >= PATIENCE {
                break;
            }
            self.sweep((s % 2 == 0) == first_down);
            self.pin();
            if exchange {
                self.transpose();
            }
            let count = self.crossings();
            if count < best_count {
                best_count = count;
                best = self.order.clone();
                stale = 0;
            } else {
                stale += 1;
            }
        }
        self.order = best;
        best_count
    }

    /// Crossings between the links of `u` and of `v` when `u` lies left of `v` in their rank.
    pub(super) fn pair_crossings(&self, u: usize, v: usize, pos: &[usize]) -> usize {
        let mut count = 0;
        for links in [&self.up, &self.down] {
            for &a in &links[u] {
                count += links[v].iter().filter(|&&b| pos[a] > pos[b]).count();
            }
        }
        count
    }

    /// Exchanges 2 neighbours of one group wherever that removes crossings, until none does.
    pub(super) fn transpose(&mut self) {
        let mut pos = vec![0usize; self.items.len()];
        for rank in &self.order {
            for (p, &i) in rank.iter().enumerate() {
                pos[i] = p;
            }
        }
        for _ in 0..self.items.len().max(4) {
            let mut improved = false;
            for r in 0..self.order.len() {
                for i in 0..self.order[r].len().saturating_sub(1) {
                    let (u, v) = (self.order[r][i], self.order[r][i + 1]);
                    let border = |x: usize| matches!(self.items[x].kind, Kind::Border(..));
                    if border(u) || border(v) || self.items[u].group != self.items[v].group {
                        continue;
                    }
                    if self.pair_crossings(v, u, &pos) < self.pair_crossings(u, v, &pos) {
                        self.order[r].swap(i, i + 1);
                        pos[u] = i + 1;
                        pos[v] = i;
                        improved = true;
                    }
                }
            }
            if !improved {
                break;
            }
        }
    }
}

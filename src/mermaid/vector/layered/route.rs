//! The ports and the routes of the edges, once the nodes have their places.

use super::*;

/// What the routes need of the layout: the input, the items, the ranks and the places.
pub(super) struct Router<'a> {
    pub(super) input: &'a Input,
    pub(super) lay: &'a Layout<'a>,
    pub(super) edge_items: &'a Vec<Vec<usize>>,
    pub(super) oriented: &'a Vec<(usize, usize)>,
    pub(super) flat: &'a Vec<Option<bool>>,
    pub(super) back: &'a Vec<bool>,
    pub(super) node_item: &'a Vec<usize>,
    pub(super) ends: &'a Vec<Vec<(usize, bool)>>,
    pub(super) along_of: &'a Vec<f64>,
    pub(super) nodes: &'a Vec<P>,
    pub(super) pos: &'a Vec<f64>,
    pub(super) band: &'a Vec<f64>,
    pub(super) heads: &'a Vec<(f64, f64)>,
    pub(super) rank: &'a Vec<usize>,
    /// Where edges may bend between a rank and the next.
    pub(super) zone: &'a dyn Fn(usize) -> (f64, f64),
    /// Where an end of an edge aims across.
    pub(super) aim: &'a dyn Fn(usize, bool) -> f64,
}

impl Router<'_> {
    /// The routes of the edges and the places of their labels, with the bends at `x` and the
    /// sides across the flow that the edges leave by and arrive by.
    pub(super) fn routes(
        &self,
        x: &[f64],
        side_out: &[Option<f64>],
        side_in: &[Option<f64>],
    ) -> Routes {
        let Router {
            input,
            lay,
            edge_items,
            oriented,
            flat,
            back,
            node_item,
            ends,
            along_of,
            nodes,
            pos,
            band,
            heads,
            rank,
            zone,
            aim: _,
        } = *self;
        let mut x = x.to_vec();
        let mut port = self.even_ports(side_out, side_in);
        let lateral = self.laterals(&x, side_out, side_in);
        // Whether the items `inner` of the lane of `e` fit at `to` across between their
        // neighbours, its sides across the flow still clear and long enough for a turn.
        let lane_fits = |x: &[f64], e: usize, inner: &[usize], to: f64| {
            let (u, v) = oriented[e];
            let side_ok = |w: usize, sign: Option<f64>, head: f64| {
                sign.is_none_or(|sign| {
                    let node = &input.nodes[w];
                    let c = x[node_item[w]];
                    (to - c) * sign + 1e-6 >= node.lo + MIN_CORNER + head
                        && lay.clear(x, rank[w], c + sign * node.lo, to, node_item[w])
                })
            };
            side_ok(u, side_out[e], heads[e].0)
                && side_ok(v, side_in[e], heads[e].1)
                && lay.room(x, inner, to)
        };
        // A lane moves onto the port it reaches, or leaves, when its ranks leave room for it.
        for (e, list) in edge_items.iter().enumerate() {
            let inner = &list[1..list.len() - 1];
            if inner.is_empty() || inner.iter().any(|&i| (x[i] - x[inner[0]]).abs() > 0.05) {
                continue;
            }
            let (u, v) = oriented[e];
            let to = if side_in[e].is_none() {
                x[node_item[v]] + port[e].1
            } else if side_out[e].is_none() {
                x[node_item[u]] + port[e].0
            } else {
                continue;
            };
            if (to - x[inner[0]]).abs() < 0.05 {
                continue;
            }
            if lane_fits(&x, e, inner, to) {
                for &i in inner {
                    x[i] = to;
                }
            }
        }
        // A port that shares its side with other ports moves onto the position across of what
        // its edge meets next, so that the edge runs straight, when its side leaves room for
        // it: the ports keep their order and 1 gap between them, and stay in the reach of the
        // side. A port alone on its side keeps the middle.
        let hard = |i: usize| lay.items[i].label || band[lay.items[i].rank] > 0.0;
        // The moves that bring the port of `e` at its upper or lower node to `to`: the ports
        // beyond it on its side give way when their edges do not run straight yet.
        let shift = |port: &[(f64, f64)],
                     fixed: &std::collections::BTreeSet<(usize, bool)>,
                     e: usize,
                     leaving: bool,
                     to: f64|
         -> Option<Vec<(usize, f64)>> {
            let v = if leaving {
                oriented[e].0
            } else {
                oriented[e].1
            };
            let room = port_room(&input.nodes[v]);
            let of = |f: usize| if leaving { port[f].0 } else { port[f].1 };
            let mut side: Vec<usize> = ends[v]
                .iter()
                .filter(|&&(f, lower)| {
                    lower == leaving
                        && if lower {
                            side_out[f].is_none()
                        } else {
                            side_in[f].is_none()
                        }
                })
                .map(|&(f, _)| f)
                .collect();
            if side.len() < 2 || to.abs() > room {
                return None;
            }
            side.sort_by(|&f, &g| of(f).total_cmp(&of(g)).then(f.cmp(&g)));
            let k = side.iter().position(|&f| f == e)?;
            let mut moves = vec![(e, to)];
            // Up the side, then down: each port at least 1 gap past the one before it.
            let mut at = to;
            for &f in &side[k + 1..] {
                let want = of(f).max(at + MIN_GAP);
                if want > of(f) + 0.05 && (fixed.contains(&(f, leaving)) || want > room) {
                    return None;
                }
                moves.push((f, want));
                at = want;
            }
            at = to;
            for &f in side[..k].iter().rev() {
                let want = of(f).min(at - MIN_GAP);
                if want < of(f) - 0.05 && (fixed.contains(&(f, leaving)) || want < -room) {
                    return None;
                }
                moves.push((f, want));
                at = want;
            }
            Some(moves)
        };
        let apply = |port: &mut [(f64, f64)], moves: &[(usize, f64)], leaving: bool| {
            for &(f, to) in moves {
                if leaving {
                    port[f].0 = to;
                } else {
                    port[f].1 = to;
                }
            }
        };
        // Where an end of an edge goes next across: its first hard bend, or the other end.
        let next = |port: &[(f64, f64)], x: &[f64], e: usize, leaving: bool| -> f64 {
            let list = &edge_items[e];
            let (u, v) = oriented[e];
            let inner = &list[1..list.len() - 1];
            if leaving {
                match inner.iter().copied().find(|&i| hard(i)) {
                    Some(i) => x[i],
                    None if side_in[e].is_none() => x[node_item[v]] + port[e].1,
                    None => x[inner[inner.len() - 1]],
                }
            } else {
                match inner.iter().rev().copied().find(|&i| hard(i)) {
                    Some(i) => x[i],
                    None if side_out[e].is_none() => x[node_item[u]] + port[e].0,
                    None => x[inner[0]],
                }
            }
        };
        // The ends that already meet what comes next stay where they are.
        let mut fixed = std::collections::BTreeSet::new();
        for e in (0..edge_items.len()).filter(|&e| flat[e].is_none()) {
            let (u, v) = oriented[e];
            if side_out[e].is_none()
                && (x[node_item[u]] + port[e].0 - next(&port, &x, e, true)).abs() < 0.05
            {
                fixed.insert((e, true));
            }
            if side_in[e].is_none()
                && (x[node_item[v]] + port[e].1 - next(&port, &x, e, false)).abs() < 0.05
            {
                fixed.insert((e, false));
            }
        }
        for _ in 0..2 {
            for (e, list) in edge_items.iter().enumerate() {
                if flat[e].is_some() {
                    continue;
                }
                let (u, v) = oriented[e];
                let (xu, xv) = (x[node_item[u]], x[node_item[v]]);
                let inner = &list[1..list.len() - 1];
                // A straight lane between 2 ports along the flow: the ports and the lane take
                // one position across, the first of the lane, the start, the end or halfway
                // that all 3 can reach.
                let uniform = inner.iter().all(|&i| (x[i] - x[inner[0]]).abs() < 0.05);
                if uniform && side_out[e].is_none() && side_in[e].is_none() {
                    let (a, b) = (xu + port[e].0, xv + port[e].1);
                    let lane_now = inner.first().map(|&i| x[i]);
                    let straight =
                        (a - b).abs() < 0.05 && lane_now.is_none_or(|l| (l - a).abs() < 0.05);
                    if straight {
                        continue;
                    }
                    let mut options = vec![a, b, (a + b) / 2.0];
                    if let Some(l) = lane_now {
                        options.insert(0, l);
                    }
                    for c in options {
                        let start = if (c - a).abs() < 0.05 {
                            Some(Vec::new())
                        } else {
                            shift(&port, &fixed, e, true, c - xu)
                        };
                        let end = if (c - b).abs() < 0.05 {
                            Some(Vec::new())
                        } else {
                            shift(&port, &fixed, e, false, c - xv)
                        };
                        let lane = lane_now
                            .is_none_or(|l| (l - c).abs() < 0.05 || lane_fits(&x, e, inner, c));
                        if let (Some(start), Some(end), true) = (start, end, lane) {
                            apply(&mut port, &start, true);
                            apply(&mut port, &end, false);
                            port[e] = (c - xu, c - xv);
                            for &i in inner {
                                x[i] = c;
                            }
                            fixed.insert((e, true));
                            fixed.insert((e, false));
                            break;
                        }
                    }
                    continue;
                }
                for leaving in [true, false] {
                    let lateral = if leaving { side_out[e] } else { side_in[e] };
                    if lateral.is_some() {
                        continue;
                    }
                    let (centre, now) = if leaving {
                        (xu, xu + port[e].0)
                    } else {
                        (xv, xv + port[e].1)
                    };
                    let to = next(&port, &x, e, leaving);
                    if (to - now).abs() > 0.05
                        && let Some(moves) = shift(&port, &fixed, e, leaving, to - centre)
                    {
                        apply(&mut port, &moves, leaving);
                        fixed.insert((e, leaving));
                    }
                }
            }
        }
        // Arrivals of the same look on one side along the flow share a port: an edge that steps
        // joins the port of an edge of its look that arrives straight, the nearest, when no
        // other port lies between and their sources do not lie on both sides of that port.
        for (v, at) in ends.iter().enumerate() {
            let side: Vec<usize> = at
                .iter()
                .filter(|&&(e, lower)| !lower && side_in[e].is_none())
                .map(|&(e, _)| e)
                .collect();
            let forward: Vec<usize> = side.iter().copied().filter(|&e| !back[e]).collect();
            let xv = x[node_item[v]];
            let straight = |port: &[(f64, f64)], f: usize| {
                (next(port, &x, f, false) - xv - port[f].1).abs() < 0.05
            };
            for &e in &forward {
                if straight(&port, e) {
                    continue;
                }
                let from = next(&port, &x, e, false);
                let join = forward
                    .iter()
                    .copied()
                    .filter(|&f| f != e && input.edges[f].look == input.edges[e].look)
                    .filter(|&f| straight(&port, f))
                    .filter(|&f| {
                        let at = xv + port[f].1;
                        let side = |g: usize| x[node_item[oriented[g].0]] - at;
                        side(e) * side(f) >= -1e-6 || side(f).abs() < 0.5
                    })
                    .min_by(|&f, &g| {
                        let d = |h: usize| (xv + port[h].1 - from).abs();
                        d(f).total_cmp(&d(g)).then(f.cmp(&g))
                    });
                let Some(f) = join else { continue };
                let to = port[f].1;
                let (lo, hi) = ((from - xv).min(to), (from - xv).max(to));
                let between = side.iter().any(|&g| {
                    g != e
                        && (port[g].1 - to).abs() >= 0.05
                        && port[g].1 > lo + 0.05
                        && port[g].1 < hi - 0.05
                });
                if !between {
                    port[e].1 = to;
                }
            }
        }
        // The height of the run across of each square step, by edge and step.
        let mut tracks: BTreeMap<(usize, usize), f64> = BTreeMap::new();
        // The square steps of the first pass: edge, step, start, end, top and bottom of the gap.
        let mut steps: Vec<Step> = Vec::new();
        let mut edges = Vec::new();
        let mut labels = Vec::new();
        // The straight diagonals that may replace square routes, with their labels.
        let mut diagonals: Vec<(usize, Vec<Seg>, Option<P>)> = Vec::new();
        for pass in 0..2 {
            edges = Vec::with_capacity(input.edges.len());
            labels = Vec::with_capacity(input.edges.len());
            for (e, list) in edge_items.iter().enumerate() {
                let (u, v) = oriented[e];
                if let Some(after) = flat[e] {
                    // A side branch: straight across from the side of its parent to the facing side
                    // of the branch, the label on the line.
                    let sign = if after { 1.0 } else { -1.0 };
                    let y = nodes[u].1;
                    let from = nodes[u].0 + sign * side_across(&input.nodes[u], along_of[u], 0.0);
                    let to = nodes[v].0 - sign * input.nodes[v].lo;
                    let mut segs = vec![Seg::Move((from, y))];
                    let mut label = None;
                    if let Some((across, _)) = input.edges[e].label {
                        let mid = (from + to) / 2.0;
                        let cut = across / 2.0 + LABEL_CUT;
                        segs.push(Seg::Line((mid - sign * cut, y)));
                        segs.push(Seg::Move((mid + sign * cut, y)));
                        label = Some((mid, y));
                    }
                    segs.push(Seg::Line((to, y)));
                    edges.push(clean(segs));
                    labels.push(label);
                    continue;
                }
                let nu = &input.nodes[u];
                let nv = &input.nodes[v];
                let (ux, vx) = (nodes[u].0 + port[e].0, nodes[v].0 + port[e].1);
                let start = (
                    ux,
                    nodes[u].1 + reach(nu.outline, 2.0 * nu.lo, along_of[u], port[e].0),
                );
                let end = (
                    vx,
                    nodes[v].1 - reach(nv.outline, 2.0 * nv.lo, along_of[v], port[e].1),
                );
                let mut segs = vec![Seg::Move(start)];
                let mut at = start;
                let mut label = None;
                if let Some(sign) = side_out[e] {
                    // Out of the side, one rounded turn, then down the lane.
                    let lane = x[list[1]];
                    let y = nodes[u].1 + lateral[e].0;
                    let from_x = nodes[u].0 + sign * side_across(nu, along_of[u], lateral[e].0);
                    let room = (lane - from_x).abs();
                    let r = CORNER.min(room - heads[e].0).max(MIN_CORNER).min(room);
                    segs = vec![
                        Seg::Move((from_x, y)),
                        Seg::Line((lane - sign * r, y)),
                        Seg::Curve(
                            (lane - sign * r * (1.0 - ARC), y),
                            (lane, y + r * (1.0 - ARC)),
                            (lane, y + r),
                        ),
                    ];
                    at = (lane, y + r);
                }
                // The rank of the band that the path last left.
                let mut from = rank[u];
                for (k, &i) in list.iter().enumerate().skip(1) {
                    let item = &lay.items[i];
                    let r = item.rank;
                    let last = k == list.len() - 1;
                    // A bend in a band that holds no label nor node is not a point the path must go
                    // through: one curve joins the bands around it.
                    let lane_end = side_in[e].is_some() && k == list.len() - 2;
                    let lane_start = side_out[e].is_some() && k == 1;
                    if !last && !item.label && band[r] <= 0.0 && !lane_end && !lane_start {
                        continue;
                    }
                    if last && let Some(sign) = side_in[e] {
                        // Down the lane, one rounded turn, then into the side.
                        let y = nodes[v].1 + lateral[e].1;
                        let to_x = nodes[v].0 + sign * side_across(nv, along_of[v], lateral[e].1);
                        let room = (at.0 - to_x).abs();
                        let r = CORNER.min(room - heads[e].1).max(MIN_CORNER).min(room);
                        segs.push(Seg::Line((at.0, y - r)));
                        segs.push(Seg::Curve(
                            (at.0, y - r * (1.0 - ARC)),
                            (at.0 - sign * r * (1.0 - ARC), y),
                            (at.0 - sign * r, y),
                        ));
                        segs.push(Seg::Line((to_x, y)));
                        break;
                    }
                    let target_x = if last { end.0 } else { x[i] };
                    let (z0, _) = zone(from);
                    let z0 = z0.max(at.1);
                    let (_, z1) = zone(r - 1);
                    // An end with a head arrives on a straight line as long as the head.
                    let z0 = if segs.len() == 1 && heads[e].0 > 0.0 {
                        z0.max((start.1 + heads[e].0 + 2.0).min(z1 - 4.0))
                    } else {
                        z0
                    };
                    let z1 = if last && heads[e].1 > 0.0 {
                        z1.min((end.1 - heads[e].1 - 2.0).max(z0 + 4.0))
                    } else {
                        z1
                    };
                    let cut = item.along / 2.0 + LABEL_CUT;
                    let top = if last {
                        end.1
                    } else if item.label {
                        (pos[r] - band[r] / 2.0).min(pos[r] - cut)
                    } else {
                        pos[r] - band[r] / 2.0
                    };
                    if (target_x - at.0).abs() < 0.05 {
                        segs.push(Seg::Line((at.0, top)));
                    } else {
                        // Square: down to the middle of the gap, across, down, the corners rounded.
                        let dx = target_x - at.0;
                        let sign = dx.signum();
                        if pass == 0 {
                            steps.push(Step {
                                edge: e,
                                step: k,
                                from: at.0,
                                to: target_x,
                                z0,
                                z1,
                            });
                        }
                        let zm = tracks.get(&(e, k)).copied().unwrap_or((z0 + z1) / 2.0);
                        let r = CORNER
                            .min(dx.abs() / 2.0)
                            .min(zm - z0)
                            .min(z1 - zm)
                            .max(0.0);
                        let k = r * (1.0 - ARC);
                        segs.push(Seg::Line((at.0, zm - r)));
                        segs.push(Seg::Curve(
                            (at.0, zm - k),
                            (at.0 + sign * k, zm),
                            (at.0 + sign * r, zm),
                        ));
                        segs.push(Seg::Line((target_x - sign * r, zm)));
                        segs.push(Seg::Curve(
                            (target_x - sign * k, zm),
                            (target_x, zm + k),
                            (target_x, zm + r),
                        ));
                        segs.push(Seg::Line((target_x, top)));
                    }
                    if last {
                        break;
                    }
                    if item.label {
                        segs.push(Seg::Line((x[i], pos[r] - cut)));
                        segs.push(Seg::Move((
                            x[i],
                            (pos[r] + cut).max(pos[r] + band[r] / 2.0),
                        )));
                        label = Some((x[i], pos[r]));
                    } else {
                        segs.push(Seg::Line((x[i], pos[r] + band[r] / 2.0)));
                    }
                    at = (
                        x[i],
                        (pos[r] + band[r] / 2.0).max(pos[r] + if item.label { cut } else { 0.0 }),
                    );
                    from = r;
                }
                // An edge between 2 neighbouring ranks of nodes runs as one straight diagonal when
                // the line between its 2 ports already lies at a multiple of 15 degrees and crosses
                // no node, label nor other edge; else it stays square.
                if side_out[e].is_none() && side_in[e].is_none() && list.len() == 3 {
                    let (dx, dy) = (end.0 - start.0, end.1 - start.1);
                    let angle = dx.abs().atan2(dy).to_degrees();
                    let steps = (angle / 15.0).round();
                    let clear = (0..lay.items.len()).all(|j| {
                        let item = &lay.items[j];
                        let near = item.rank + 1 >= rank[u] && item.rank <= rank[v];
                        let mine = j == list[0] || j == list[1] || j == list[2];
                        let solid = matches!(item.kind, Kind::Node(_)) || item.label;
                        if !near || mine || !solid {
                            return true;
                        }
                        let (l, r) = (x[j] - item.lo, x[j] + item.hi);
                        let (t, b) = (
                            pos[item.rank] - item.along / 2.0,
                            pos[item.rank] + item.along / 2.0,
                        );
                        (0..=24).all(|k| {
                            let f = f64::from(k) / 24.0;
                            let (px, py) = (start.0 + dx * f, start.1 + dy * f);
                            px <= l || px >= r || py <= t || py >= b
                        })
                    });
                    if dx.abs() >= 0.5
                        && dy > 0.0
                        && (1.0..=FLATTEST / 15.0).contains(&steps)
                        && (angle - steps * 15.0).abs() < 0.5
                        && clear
                    {
                        let mut line = vec![Seg::Move(start)];
                        let mut diagonal_label = label;
                        if lay.items[list[1]].label {
                            let mid = ((start.0 + end.0) / 2.0, (start.1 + end.1) / 2.0);
                            let len = (dx * dx + dy * dy).sqrt();
                            let item = &lay.items[list[1]];
                            let cut = (item.lo.max(item.along / 2.0) + LABEL_CUT) / len;
                            line.push(Seg::Line((mid.0 - dx * cut, mid.1 - dy * cut)));
                            line.push(Seg::Move((mid.0 + dx * cut, mid.1 + dy * cut)));
                            diagonal_label = Some(mid);
                        }
                        line.push(Seg::Line(end));
                        let line = clean(line);
                        diagonals.push((
                            e,
                            if back[e] { reverse(&line) } else { line },
                            diagonal_label,
                        ));
                    }
                }
                let segs = clean(segs);
                edges.push(if back[e] { reverse(&segs) } else { segs });
                labels.push(label);
            }
            // Each diagonal takes the place of its square route when it crosses no other edge.
            for (e, line, label) in std::mem::take(&mut diagonals) {
                let mine = polylines(&line);
                let free = edges
                    .iter()
                    .enumerate()
                    .filter(|&(f, _)| f != e)
                    .all(|(_, other)| {
                        polylines(other).iter().all(|run| {
                            mine.iter().all(|m| {
                                m.windows(2).all(|a| {
                                    run.windows(2).all(|b| !crosses(a[0], a[1], b[0], b[1]))
                                })
                            })
                        })
                    });
                if free {
                    edges[e] = line;
                    labels[e] = label;
                }
            }
            if pass == 0 {
                tracks = square_tracks(&steps);
            }
        }
        (edges, labels)
    }

    /// The ports on the sides along the flow, as offsets across from the centre of each end:
    /// n ports at (k + 1) / (n + 1) of the side, in the order of what they lead to, the middle
    /// for one alone.
    fn even_ports(&self, side_out: &[Option<f64>], side_in: &[Option<f64>]) -> Vec<(f64, f64)> {
        let (input, ends, aim) = (self.input, self.ends, self.aim);
        let mut port = vec![(0.0f64, 0.0f64); input.edges.len()];
        for (v, at) in ends.iter().enumerate() {
            for low_side in [true, false] {
                let mut side: Vec<(f64, usize)> = at
                    .iter()
                    .filter(|&&(_, lower)| lower == low_side)
                    .filter(|&&(e, lower)| {
                        if lower {
                            side_out[e].is_none()
                        } else {
                            side_in[e].is_none()
                        }
                    })
                    .map(|&(e, lower)| (aim(e, lower), e))
                    .collect();
                side.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
                let node = &input.nodes[v];
                let k = side.len();
                for (i, &(_, e)) in side.iter().enumerate() {
                    let dx = -node.lo + 2.0 * node.lo * (i + 1) as f64 / (k + 1) as f64;
                    if low_side {
                        port[e].0 = dx;
                    } else {
                        port[e].1 = dx;
                    }
                }
            }
        }
        port
    }

    /// The ports on the sides across the flow, as offsets along from the centre of each end: n
    /// edges at (k + 1) / (n + 1) of the side. Arrivals from the inner lane first, departures to
    /// the outer lane first, from the low end, so that the lanes do not cross.
    fn laterals(
        &self,
        x: &[f64],
        side_out: &[Option<f64>],
        side_in: &[Option<f64>],
    ) -> Vec<(f64, f64)> {
        let (input, edge_items, oriented) = (self.input, self.edge_items, self.oriented);
        let (node_item, along_of) = (self.node_item, self.along_of);
        let mut lateral = vec![(0.0f64, 0.0f64); input.edges.len()];
        let mut groups: BTreeMap<(usize, bool, bool), Vec<(f64, usize)>> = BTreeMap::new();
        for (e, list) in edge_items.iter().enumerate() {
            if let Some(sign) = side_out[e] {
                let u = oriented[e].0;
                let d = (x[list[1]] - x[node_item[u]]).abs();
                groups
                    .entry((u, sign > 0.0, true))
                    .or_default()
                    .push((-d, e));
            }
            if let Some(sign) = side_in[e] {
                let v = oriented[e].1;
                let d = (x[list[list.len() - 2]] - x[node_item[v]]).abs();
                groups
                    .entry((v, sign > 0.0, false))
                    .or_default()
                    .push((d, e));
            }
        }
        // Arrivals of the same look on one side share their slot, and so their last run.
        for (&(v, _, leaving), list) in &mut groups {
            list.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            let half = along_of[v] / 2.0;
            let class = |e: usize| if leaving { e } else { input.edges[e].look };
            let mut classes: Vec<usize> = Vec::new();
            for &(_, e) in list.iter() {
                if !classes.contains(&class(e)) {
                    classes.push(class(e));
                }
            }
            let k = classes.len();
            for &(_, e) in list.iter() {
                let i = classes.iter().position(|&c| c == class(e)).unwrap_or(0);
                let dy = -half + 2.0 * half * (i + 1) as f64 / (k + 1) as f64;
                if leaving {
                    lateral[e].0 = dy;
                } else {
                    lateral[e].1 = dy;
                }
            }
        }
        lateral
    }
}

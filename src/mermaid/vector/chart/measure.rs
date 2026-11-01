//! What a drawing is measured by: crossings, turns, steps, ports and the main path.

use super::*;

/// The crossings, the bends and the length of the edges of the drawing of `flow`. A bend is a
/// turn: a step across the flow makes 2.
pub fn measure(flow: &Flow, kind: Kind) -> Option<Counted> {
    let d = choose(flow, kind)?;
    let runs: Vec<Vec<Vec<P>>> = d.paths.iter().map(|p| sample(p)).collect();
    let mut crossings = 0;
    for (i, a) in runs.iter().enumerate() {
        for b in &runs[i + 1..] {
            for ra in a {
                for rb in b {
                    for sa in ra.windows(2) {
                        for sb in rb.windows(2) {
                            if intersect(sa[0], sa[1], sb[0], sb[1]) {
                                crossings += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    let turns: Vec<usize> = d
        .paths
        .iter()
        .map(|p| {
            let mut at = (0.0, 0.0);
            p.iter()
                .map(|s| {
                    let turns = match *s {
                        // A step across makes 2 turns, a quarter turn 1.
                        Seg::Curve(c1, c2, q)
                            if (q.0 - at.0).abs() > 0.5 && (q.1 - at.1).abs() > 0.5 =>
                        {
                            let (a, b) = ((c1.0 - at.0, c1.1 - at.1), (q.0 - c2.0, q.1 - c2.1));
                            if (a.0 * b.1 - a.1 * b.0).abs() < 1e-6 {
                                2
                            } else {
                                1
                            }
                        }
                        _ => 0,
                    };
                    at = end_point(s);
                    turns
                })
                .sum::<usize>()
        })
        .collect();
    let bends = turns.iter().sum();
    // An S-curve is a step: a curve whose ends run the same way, offset across.
    let s_curves = d
        .paths
        .iter()
        .filter(|p| {
            let mut at = (0.0, 0.0);
            p.iter().any(|s| {
                let step = match *s {
                    Seg::Curve(c1, c2, q)
                        if (q.0 - at.0).abs() > 0.5 && (q.1 - at.1).abs() > 0.5 =>
                    {
                        let (a, b) = ((c1.0 - at.0, c1.1 - at.1), (q.0 - c2.0, q.1 - c2.1));
                        (a.0 * b.1 - a.1 * b.0).abs() < 1e-6
                    }
                    _ => false,
                };
                at = end_point(s);
                step
            })
        })
        .count();
    // Straight runs of 2 edges side by side, closer than the gap of 2 ports, along more than the
    // height of the lowest node.
    let low = d
        .ends
        .iter()
        .flatten()
        .map(|e| e.2.min(e.1))
        .fold(f64::INFINITY, f64::min);
    let lines: Vec<Vec<(P, P)>> = d
        .paths
        .iter()
        .map(|p| {
            let mut out = Vec::new();
            let mut at = (0.0, 0.0);
            for s in p {
                if let Seg::Line(q) = *s {
                    out.push((at, q));
                }
                at = end_point(s);
            }
            out
        })
        .collect();
    let mut close = 0;
    for (i, a) in lines.iter().enumerate() {
        for b in &lines[i + 1..] {
            for &(p, q) in a {
                for &(r, t) in b {
                    let vertical = |u: P, v: P| (u.0 - v.0).abs() < 0.5;
                    let horizontal = |u: P, v: P| (u.1 - v.1).abs() < 0.5;
                    let (gap, overlap) = if vertical(p, q) && vertical(r, t) {
                        let lo = p.1.min(q.1).max(r.1.min(t.1));
                        let hi = p.1.max(q.1).min(r.1.max(t.1));
                        ((p.0 - r.0).abs(), hi - lo)
                    } else if horizontal(p, q) && horizontal(r, t) {
                        let lo = p.0.min(q.0).max(r.0.min(t.0));
                        let hi = p.0.max(q.0).min(r.0.max(t.0));
                        ((p.1 - r.1).abs(), hi - lo)
                    } else {
                        continue;
                    };
                    if gap > 0.5 && gap < layered::MIN_GAP && overlap > low {
                        close += 1;
                    }
                }
            }
        }
    }
    // How unevenly the ports of each side share it: the largest difference between 2 gaps, the
    // corners counted as ends.
    let mut sides: std::collections::BTreeMap<(i64, i64, char), SidePorts> =
        std::collections::BTreeMap::new();
    for (p, ends) in d.paths.iter().zip(&d.ends) {
        let tips = [
            (
                end_point(&p[0]),
                p.get(1).map_or(end_point(&p[0]), |s| match *s {
                    Seg::Curve(c1, _, _) => c1,
                    s => end_point(&s),
                }),
                ends[0],
            ),
            (
                end_point(&p[p.len() - 1]),
                match p[p.len() - 1] {
                    Seg::Curve(_, c2, _) => c2,
                    _ => end_point(&p[p.len() - 2]),
                },
                ends[1],
            ),
        ];
        for (q, next, (c, w, h)) in tips {
            let across = (next.1 - q.1).abs() < 1e-6;
            let key = |v: f64| (v * 10.0).round() as i64;
            let entry = if across {
                let side = if q.0 < c.0 { 'w' } else { 'e' };
                sides.entry((key(c.0), key(c.1), side)).or_insert((
                    c.1 - h / 2.0,
                    c.1 + h / 2.0,
                    Vec::new(),
                ))
            } else {
                let side = if q.1 < c.1 { 'n' } else { 's' };
                sides.entry((key(c.0), key(c.1), side)).or_insert((
                    c.0 - w / 2.0,
                    c.0 + w / 2.0,
                    Vec::new(),
                ))
            };
            entry.2.push(if across { q.1 } else { q.0 });
        }
    }
    let mut uneven: f64 = 0.0;
    for (lo, hi, mut ports) in sides.into_values() {
        ports.sort_by(f64::total_cmp);
        let mut marks = vec![lo];
        marks.extend(ports);
        marks.push(hi);
        let gaps: Vec<f64> = marks.windows(2).map(|w| w[1] - w[0]).collect();
        let most = gaps.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let least = gaps.iter().copied().fold(f64::INFINITY, f64::min);
        uneven = uneven.max(most - least);
    }
    // Steps shorter than `JOG` between 2 parallel straight runs of one edge.
    let jogs: usize = d
        .paths
        .iter()
        .map(|p| {
            let mut count = 0;
            // The last straight run along an axis: whether it is vertical, and its position.
            let mut last: Option<(bool, f64)> = None;
            let mut at = (0.0, 0.0);
            for s in p {
                if let Seg::Line(q) = *s {
                    let (dx, dy) = ((q.0 - at.0).abs(), (q.1 - at.1).abs());
                    let run = if dx < 0.5 && dy >= 0.5 {
                        Some((true, q.0))
                    } else if dy < 0.5 && dx >= 0.5 {
                        Some((false, q.1))
                    } else {
                        None
                    };
                    if let (Some((v, c)), Some((w, d))) = (run, last)
                        && v == w
                        && (c - d).abs() >= 0.5
                        && (c - d).abs() < JOG
                    {
                        count += 1;
                    }
                    if run.is_some() {
                        last = run;
                    }
                }
                if let Seg::Move(_) = s {
                    last = None;
                }
                at = end_point(s);
            }
            count
        })
        .sum();
    // Edges whose last run and head coincide with those of another edge.
    let finals: Vec<(P, P)> = d
        .paths
        .iter()
        .map(|p| {
            let end = end_point(&p[p.len() - 1]);
            let before = match p[p.len() - 1] {
                Seg::Curve(_, c2, _) => c2,
                _ => p.len().checked_sub(2).map_or(end, |k| end_point(&p[k])),
            };
            (end, before)
        })
        .collect();
    let near = |a: P, b: P| (a.0 - b.0).abs() < 0.5 && (a.1 - b.1).abs() < 0.5;
    let merged = finals
        .iter()
        .enumerate()
        .filter(|&(i, &(end, before))| {
            finals.iter().enumerate().any(|(j, &(e2, b2))| {
                let (u, v) = (
                    (end.0 - before.0, end.1 - before.1),
                    (e2.0 - b2.0, e2.1 - b2.1),
                );
                i != j
                    && near(end, e2)
                    && (u.0 * v.1 - u.1 * v.0).abs() < 0.5
                    && u.0 * v.0 + u.1 * v.1 > 0.0
            })
        })
        .count();
    // Edges that mix diagonal runs with runs along or across the flow, or with curves.
    let mixed = d
        .paths
        .iter()
        .filter(|p| {
            let (mut diagonal, mut square) = (false, false);
            let mut at = (0.0, 0.0);
            for s in p.iter() {
                match *s {
                    Seg::Line(q) => {
                        let (dx, dy) = ((q.0 - at.0).abs(), (q.1 - at.1).abs());
                        if dx >= 0.5 && dy >= 0.5 {
                            diagonal = true;
                        } else if dx >= 0.5 || dy >= 0.5 {
                            square = true;
                        }
                    }
                    Seg::Curve(..) => square = true,
                    Seg::Move(_) => {}
                }
                at = end_point(s);
            }
            diagonal && square
        })
        .count();
    // Diagonal runs, and their angles.
    let (mut diagonals, mut skewed) = (0, 0);
    for (p, q) in lines.iter().flatten() {
        let (dx, dy) = ((q.0 - p.0).abs(), (q.1 - p.1).abs());
        if dx < 0.5 || dy < 0.5 {
            continue;
        }
        diagonals += 1;
        let angle = dx.atan2(dy).to_degrees();
        if (angle - (angle / 15.0).round() * 15.0).abs() > 0.5 {
            skewed += 1;
        }
    }
    // Ranks of boxes of text whose lengths along the flow differ.
    let horizontal = flow.direction.horizontal();
    let mut ranks: std::collections::BTreeMap<i64, (f64, f64)> = std::collections::BTreeMap::new();
    for &(c, w, h, text) in &d.boxes {
        if !text {
            continue;
        }
        let (key, along) = if horizontal { (c.0, w) } else { (c.1, h) };
        let e = ranks
            .entry((key * 10.0).round() as i64)
            .or_insert((along, along));
        *e = (e.0.min(along), e.1.max(along));
    }
    let ragged = ranks.values().filter(|r| r.1 - r.0 > 0.5).count();
    // The main path: the edge out of each node toward its longest chain below; the side
    // branches beside their parents.
    let links: Vec<(usize, &crate::mermaid::graph::Edge)> = flow
        .edges
        .iter()
        .enumerate()
        .filter(|(_, e)| e.from != e.to)
        .collect();
    let edges_in: Vec<EdgeIn> = links
        .iter()
        .map(|(_, e)| EdgeIn {
            from: e.from,
            to: e.to,
            label: None,
            heads: (0.0, 0.0),
            look: e.look(&flow.edges),
        })
        .collect();
    let back = layered::back_edges(flow.nodes.len(), &edges_in);
    let oriented: Vec<(usize, usize)> = edges_in
        .iter()
        .zip(&back)
        .map(|(e, &b)| if b { (e.to, e.from) } else { (e.from, e.to) })
        .collect();
    let (branches, main) = layered::main_path(flow.nodes.len(), &edges_in, &oriented, &back);
    let (mut spine, mut aligned, mut across) = (0, 0, 0);
    for (k, segs) in d.paths.iter().enumerate() {
        if back[k] {
            continue;
        }
        let first = end_point(&segs[0]);
        let last = end_point(&segs[segs.len() - 1]);
        let lines = segs.iter().all(|s| !matches!(s, Seg::Curve(..)));
        let (along_same, across_same) = if horizontal {
            (
                (first.1 - last.1).abs() < 0.5,
                (first.0 - last.0).abs() < 0.5,
            )
        } else {
            (
                (first.0 - last.0).abs() < 0.5,
                (first.1 - last.1).abs() < 0.5,
            )
        };
        if branches[k].is_some() {
            across += usize::from(lines && across_same);
            continue;
        }
        if main[k] {
            spine += 1;
            aligned += usize::from(lines && along_same);
        }
    }
    // An end off its node's vertices and side middles.
    let mut off = 0;
    for (p, ends) in d.paths.iter().zip(&d.ends) {
        let first = p.first().map(end_point);
        let last = p.last().map(end_point);
        for (point, (c, _, _)) in [first, last].into_iter().zip(ends) {
            if let Some(q) = point
                && (q.0 - c.0).abs() > 0.5
                && (q.1 - c.1).abs() > 0.5
            {
                off += 1;
            }
        }
    }
    let length = runs
        .iter()
        .flatten()
        .flat_map(|run| run.windows(2))
        .map(|w| ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt())
        .sum();
    Some(Counted {
        crossings,
        bends,
        length,
        s_curves,
        off,
        close,
        uneven,
        diagonals,
        skewed,
        ragged,
        spine,
        aligned,
        across,
        mixed,
        jogs,
        merged,
    })
}

/// The ends of a side of a node and where its ports lie along it.
type SidePorts = (f64, f64, Vec<f64>);

/// What `measure` counts in a drawing.
pub struct Counted {
    pub crossings: usize,
    /// Turns of the edges: a step across makes 2, a quarter turn 1.
    pub bends: usize,
    pub length: f64,
    /// Edges with a step: a curve whose ends run the same way, offset across.
    pub s_curves: usize,
    /// Edge ends off the vertices and the side middles of their nodes.
    pub off: usize,
    /// Pairs of straight runs of 2 edges closer than the gap of 2 ports, along more than the
    /// height of the lowest node.
    pub close: usize,
    /// The largest difference between 2 gaps of the ports of one side, the corners counted.
    pub uneven: f64,
    /// Straight runs that are neither across nor along the flow, and those of them whose angle
    /// is not a multiple of 15 degrees.
    pub diagonals: usize,
    pub skewed: usize,
    /// Ranks whose boxes of text differ in length along the flow.
    pub ragged: usize,
    /// Edges of the main path, those of them that run straight along the flow, and the side
    /// branches drawn straight across it.
    pub spine: usize,
    pub aligned: usize,
    pub across: usize,
    /// Edges that mix a diagonal with runs along or across the flow, or with curves.
    pub mixed: usize,
    /// Steps shorter than `JOG` between 2 parallel runs of one edge.
    pub jogs: usize,
    /// Edges that share their last run and their head with another edge.
    pub merged: usize,
}

/// The length under which a step between 2 parallel runs counts as a jog.
pub const JOG: f64 = 15.0;

/// Whether the segments `a`-`b` and `c`-`d` cross, each taken without its end.
fn intersect(a: P, b: P, c: P, d: P) -> bool {
    let den = (b.0 - a.0) * (d.1 - c.1) - (b.1 - a.1) * (d.0 - c.0);
    if den.abs() < 1e-9 {
        return false;
    }
    let t = ((c.0 - a.0) * (d.1 - c.1) - (c.1 - a.1) * (d.0 - c.0)) / den;
    let u = ((c.0 - a.0) * (b.1 - a.1) - (c.1 - a.1) * (b.0 - a.0)) / den;
    (0.0..1.0).contains(&t) && (0.0..1.0).contains(&u)
}

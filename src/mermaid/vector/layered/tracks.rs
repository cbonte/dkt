//! The heights of the runs across of the square steps in the gaps between ranks.

use super::*;

/// A square step of an edge across the gap between 2 bands.
pub(super) struct Step {
    pub(super) edge: usize,
    /// The index of the step along the edge.
    pub(super) step: usize,
    /// The position across where the step leaves.
    pub(super) from: f64,
    /// The position across where the step arrives.
    pub(super) to: f64,
    /// The top of the gap.
    pub(super) z0: f64,
    /// The bottom of the gap.
    pub(super) z1: f64,
}

/// The height of the run across of each square step, so that no 2 steps overlap.
///
/// The steps of one group spread evenly over the gap.
pub(super) fn square_tracks(steps: &[Step]) -> BTreeMap<(usize, usize), f64> {
    square_levels(steps)
        .into_iter()
        .zip(steps)
        .map(|((level, count), t)| {
            let z = t.z0 + (t.z1 - t.z0) * (level as f64 + 1.0) / (count as f64 + 1.0);
            ((t.edge, t.step), z)
        })
        .collect()
}

/// The level of each square step in its group, and the number of levels of the group.
///
/// Steps that go the same way and share a stretch of the same gap take separate levels, but
/// steps toward one position join there.
/// The step that leaves furthest along its way turns first, so that it crosses none of the others.
pub(super) fn square_levels(steps: &[Step]) -> Vec<(usize, usize)> {
    let n = steps.len();
    let shared = |a: &Step, b: &Step| {
        let (la, ha) = (a.from.min(a.to), a.from.max(a.to));
        let (lb, hb) = (b.from.min(b.to), b.from.max(b.to));
        (a.to > a.from) == (b.to > b.from)
            && (a.to - b.to).abs() >= 0.5
            && la < hb - 0.5
            && lb < ha - 0.5
            && a.z0 < b.z1
            && b.z0 < a.z1
    };
    let ahead = |t: &Step| (t.to - t.from).signum() * t.from;
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| ahead(&steps[b]).total_cmp(&ahead(&steps[a])));
    let mut level = vec![0usize; n];
    let mut group: Vec<usize> = (0..n).collect();
    fn root(group: &mut [usize], mut i: usize) -> usize {
        while group[i] != i {
            group[i] = group[group[i]];
            i = group[i];
        }
        i
    }
    for (k, &a) in order.iter().enumerate() {
        for &b in &order[..k] {
            if shared(&steps[a], &steps[b]) {
                level[a] = level[a].max(level[b] + 1);
                let (ra, rb) = (root(&mut group, a), root(&mut group, b));
                group[ra] = rb;
            }
        }
    }
    let mut count = vec![0usize; n];
    for (a, &l) in level.iter().enumerate() {
        let g = root(&mut group, a);
        count[g] = count[g].max(l + 1);
    }
    (0..n)
        .map(|a| (level[a], count[root(&mut group, a)]))
        .collect()
}

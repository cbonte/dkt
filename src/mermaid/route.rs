//! Orthogonal edge routing on the character grid.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use super::canvas::{Canvas, Dir, E, N, S, W};

const STEP_COST: u32 = 2;
const SHARED_STEP_COST: u32 = 1;
const TURN_COST: u32 = 6;
const OVERLAP_COST: u32 = 12;
const CROSSING_COST: u32 = 8;
const NEAR_NODE_COST: u32 = 2;

/// Cells a path must not enter, indexed `y * width + x`.
pub struct Blocked {
    w: usize,
    h: usize,
    cells: Vec<bool>,
    /// The cells next to a blocked cell, across or along.
    near: Vec<bool>,
}

impl Blocked {
    pub fn new(w: usize, h: usize) -> Blocked {
        Blocked {
            w,
            h,
            cells: vec![false; w * h],
            near: vec![false; w * h],
        }
    }

    pub fn block_rect(&mut self, x: usize, y: usize, w: usize, h: usize) {
        for cy in y..y + h {
            for cx in x..x + w {
                self.cells[cy * self.w + cx] = true;
                if cy > 0 {
                    self.near[(cy - 1) * self.w + cx] = true;
                }
                if cy + 1 < self.h {
                    self.near[(cy + 1) * self.w + cx] = true;
                }
                if cx > 0 {
                    self.near[cy * self.w + cx - 1] = true;
                }
                if cx + 1 < self.w {
                    self.near[cy * self.w + cx + 1] = true;
                }
            }
        }
    }

    fn is_blocked(&self, x: usize, y: usize) -> bool {
        self.cells[y * self.w + x]
    }

    fn near_block(&self, x: usize, y: usize) -> bool {
        self.near[y * self.w + x]
    }
}

/// The buffers of the search, kept from one route to the next. A state holds a distance only
/// when its stamp is the one of the current search.
#[derive(Default)]
pub struct Search {
    dist: Vec<u32>,
    prev: Vec<u32>,
    stamp: Vec<u32>,
    epoch: u32,
    /// The states to visit: cost, column, row and heading, the cheapest first.
    heap: BinaryHeap<Reverse<(u32, usize, usize, usize)>>,
}

impl Search {
    /// Starts a search over `size` states: every distance unknown.
    fn begin(&mut self, size: usize) {
        if self.stamp.len() != size || self.epoch == u32::MAX {
            self.dist = vec![u32::MAX; size];
            self.prev = vec![u32::MAX; size];
            self.stamp = vec![0; size];
            self.epoch = 0;
        }
        self.epoch += 1;
        self.heap.clear();
    }

    fn dist(&self, s: usize) -> u32 {
        if self.stamp[s] == self.epoch {
            self.dist[s]
        } else {
            u32::MAX
        }
    }

    fn set(&mut self, s: usize, d: u32, prev: u32) {
        self.stamp[s] = self.epoch;
        self.dist[s] = d;
        self.prev[s] = prev;
    }
}

/// Cheapest path of cells from `start`, left heading `out`, to `goal`, entered heading `arrive`,
/// where `from` is `(start, out)` and `to` is `(goal, arrive)`.
/// Paths prefer few turns, avoid nodes and cross other edges only when needed. The cells marked
/// in `share`, indexed like the canvas, hold lines the path may join and follow: they cost less
/// than an empty cell.
pub fn route(
    search: &mut Search,
    canvas: &Canvas,
    blocked: &Blocked,
    from: ((usize, usize), Dir),
    to: ((usize, usize), Dir),
    share: &[bool],
) -> Option<Vec<(usize, usize)>> {
    let ((start, out), (goal, arrive)) = (from, to);
    let (w, h) = (canvas.width(), canvas.height());
    let state = |(x, y): (usize, usize), d: Dir| (y * w + x) * 4 + d.index();
    // The ends may hold the head of another edge that shares the port.
    let open = |x: usize, y: usize| {
        (x, y) == goal || (x, y) == start || (!blocked.is_blocked(x, y) && canvas.is_free(x, y))
    };
    search.begin(w * h * 4);
    search.set(state(start, out), 0, u32::MAX);
    search
        .heap
        .push(Reverse((0u32, start.0, start.1, out.index())));
    while let Some(Reverse((cost, x, y, di))) = search.heap.pop() {
        let dir = Dir::ALL[di];
        let here = state((x, y), dir);
        if cost > search.dist(here) {
            continue;
        }
        if (x, y) == goal && dir == arrive {
            let mut path = vec![(x, y)];
            let mut s = here;
            while search.prev[s] != u32::MAX {
                s = search.prev[s] as usize;
                path.push(((s / 4) % w, (s / 4) / w));
            }
            path.reverse();
            return Some(path);
        }
        for next in Dir::ALL {
            if next == dir.opposite() {
                continue;
            }
            let at_edge = match next {
                Dir::N => y == 0,
                Dir::E => x + 1 == w,
                Dir::S => y + 1 == h,
                Dir::W => x == 0,
            };
            if at_edge {
                continue;
            }
            let (nx, ny) = next.step((x, y));
            if !open(nx, ny) {
                continue;
            }
            let shared = share.get(ny * w + nx).copied().unwrap_or(false);
            let mut step = if shared { SHARED_STEP_COST } else { STEP_COST };
            if next != dir {
                step += TURN_COST;
            }
            let lines = canvas.lines(nx, ny);
            if lines != 0 && !shared {
                let along = if matches!(next, Dir::N | Dir::S) {
                    N | S
                } else {
                    E | W
                };
                step += if lines & along != 0 {
                    OVERLAP_COST
                } else {
                    CROSSING_COST
                };
            }
            if blocked.near_block(nx, ny) {
                step += NEAR_NODE_COST;
            }
            let ns = state((nx, ny), next);
            if cost + step < search.dist(ns) {
                search.set(ns, cost + step, here as u32);
                search
                    .heap
                    .push(Reverse((cost + step, nx, ny, next.index())));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn straight_path() {
        let canvas = Canvas::new(5, 5);
        let blocked = Blocked::new(5, 5);
        let path = route(
            &mut Search::default(),
            &canvas,
            &blocked,
            ((2, 1), Dir::S),
            ((2, 3), Dir::S),
            &[],
        )
        .unwrap();
        assert_eq!(path, [(2, 1), (2, 2), (2, 3)]);
    }

    #[test]
    fn path_goes_around_a_node() {
        let canvas = Canvas::new(7, 5);
        let mut blocked = Blocked::new(7, 5);
        blocked.block_rect(2, 2, 3, 1);
        let path = route(
            &mut Search::default(),
            &canvas,
            &blocked,
            ((3, 1), Dir::S),
            ((3, 3), Dir::W),
            &[],
        )
        .unwrap();
        assert!(path.iter().all(|&(x, y)| !(y == 2 && (2..5).contains(&x))));
        assert_eq!(path.first(), Some(&(3, 1)));
        assert_eq!(path.last(), Some(&(3, 3)));
    }

    #[test]
    fn unreachable_goal() {
        let canvas = Canvas::new(3, 3);
        let mut blocked = Blocked::new(3, 3);
        blocked.block_rect(0, 1, 3, 1);
        assert_eq!(
            route(
                &mut Search::default(),
                &canvas,
                &blocked,
                ((1, 0), Dir::S),
                ((1, 2), Dir::S),
                &[]
            ),
            None
        );
    }
}

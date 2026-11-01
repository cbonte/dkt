//! Mermaid diagrams drawn as text.

mod canvas;
mod class;
mod er;
mod flow;
mod graph;
mod parse;
mod pie;
mod route;
mod sequence;
mod state;
mod vector;

use canvas::Canvas;

use crate::model::Line;
use crate::options::{Glyphs, RenderOptions};

/// A diagram as the lines of the terminal, and the grid that they were drawn from.
pub struct Drawing {
    pub lines: Vec<Line>,
    pub canvas: Canvas,
    /// The route of each edge.
    pub routes: Vec<Route>,
    /// The box of each node, left, top, width and height in cells, and whether it is a diamond.
    pub boxes: Vec<(usize, usize, usize, usize, bool)>,
    /// The source and the target of each route.
    pub links: Vec<(usize, usize)>,
    /// Whether the flow runs across the page.
    pub horizontal: bool,
}

/// The cells of an edge, the direction it leaves its source in and the one it reaches its
/// target in.
pub type Route = (Vec<(usize, usize)>, canvas::Dir, canvas::Dir);

/// The columns that a drawing for a page may take, which no diagram goes past.
const PAGE_WIDTH: usize = 1000;

/// What the drawer made of a block: a drawing on a grid, or the lines of a chart.
enum Drawn {
    Grid(Drawing),
    Chart(Vec<Line>),
}

fn draw(code: &str, width: usize, opts: &RenderOptions) -> Option<Drawn> {
    let first = code
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with("%%"))?;
    if first.trim_end_matches(';') == "sequenceDiagram" {
        return sequence::render(code, width, opts).map(Drawn::Grid);
    }
    if first.split_whitespace().next() == Some("pie") {
        return pie::render(code, width, opts).map(Drawn::Chart);
    }
    let mut flow = match first.split_whitespace().next() {
        Some("stateDiagram" | "stateDiagram-v2") => state::parse(code)?,
        Some("classDiagram" | "classDiagram-v2") => class::parse(code)?,
        Some("erDiagram") => er::parse(code)?,
        _ => parse::parse(code)?,
    };
    let drawn = flow::draw(&flow, width, opts);
    if drawn.is_none() && flow.direction.horizontal() {
        // A left-to-right layout too wide for the terminal is stacked top-down instead.
        flow.direction = graph::Direction::TopDown;
        return flow::draw(&flow, width, opts).map(Drawn::Grid);
    }
    drawn.map(Drawn::Grid)
}

/// Draws `code` within `width` columns. `None` when the diagram type or syntax is not drawn, or
/// when the drawing does not fit.
pub fn render(code: &str, width: usize, opts: &RenderOptions) -> Option<Vec<Line>> {
    match draw(code, width, opts)? {
        Drawn::Grid(drawing) => Some(drawing.lines),
        Drawn::Chart(lines) => Some(lines),
    }
}

/// Crossings and bends of the edges of a graph diagram, in the vector drawing and in the grid
/// drawing. A bend is a turn of an edge.
#[derive(Debug, Default, PartialEq)]
pub struct Measure {
    pub vector: Option<Counts>,
    pub grid: Option<Counts>,
}

/// Crossings, turns and total length of the edges of a drawing: in the units of the page for the
/// vector drawing, in cells for the grid.
#[derive(Debug, Default, PartialEq)]
pub struct Counts {
    pub crossings: usize,
    pub bends: usize,
    pub length: f64,
    /// Edges that turn twice or more.
    pub s_curves: usize,
    /// Edge ends off the vertices and the side middles of their nodes, in the vector drawing.
    pub off: usize,
    /// Straight runs of 2 edges too close side by side, in the vector drawing.
    pub close: usize,
    /// The largest difference between 2 gaps of the ports of one side, in the vector drawing.
    pub uneven: f64,
    /// Diagonal runs in the vector drawing, those off a multiple of 15 degrees, and the ranks
    /// whose boxes differ in length along the flow.
    pub diagonals: usize,
    pub skewed: usize,
    pub ragged: usize,
    /// Columns of the grid drawing.
    pub width: usize,
    /// Main-path edges, those that run straight along the flow, and side branches drawn straight
    /// across, in the vector drawing.
    pub spine: usize,
    pub aligned: usize,
    pub across: usize,
    /// Edges of the vector drawing that mix a diagonal with square runs or curves.
    pub mixed: usize,
    /// Short steps between 2 parallel runs of one edge: under the jog length in the vector
    /// drawing, of at most the grid jog length in the grid.
    pub jogs: usize,
    /// Edges that share their last run and head with another edge.
    pub merged: usize,
}

/// Counts the crossings and the bends of the edges of `code` in both drawings, the grid drawn
/// within the columns of a page.
pub fn measure(code: &str) -> Measure {
    let opts = RenderOptions {
        glyphs: Glyphs::unicode(),
        ..RenderOptions::plain(PAGE_WIDTH)
    };
    let grid = match draw(code, PAGE_WIDTH, &opts) {
        Some(Drawn::Grid(d)) if !d.routes.is_empty() => {
            let (crossings, bends) = grid_measure(&d.routes);
            let length = d.routes.iter().map(|r| r.0.len() as f64 + 1.0).sum();
            let (s_curves, ragged, uneven) = grid_shape(&d);
            let width = d
                .lines
                .iter()
                .map(|l| crate::wrap::segments_width(l))
                .max()
                .unwrap_or(0);
            Some(Counts {
                crossings,
                bends,
                length,
                s_curves,
                off: 0,
                close: 0,
                uneven,
                diagonals: 0,
                skewed: 0,
                ragged,
                width,
                spine: 0,
                aligned: 0,
                across: 0,
                mixed: 0,
                jogs: grid_jogs(&d),
                merged: grid_merged(&d),
            })
        }
        _ => None,
    };
    Measure {
        vector: vector::measure(code).map(|c| Counts {
            crossings: c.crossings,
            bends: c.bends,
            length: c.length,
            s_curves: c.s_curves,
            off: c.off,
            close: c.close,
            uneven: c.uneven,
            diagonals: c.diagonals,
            skewed: c.skewed,
            ragged: c.ragged,
            width: 0,
            spine: c.spine,
            aligned: c.aligned,
            across: c.across,
            mixed: c.mixed,
            jogs: c.jogs,
            merged: c.merged,
        }),
        grid,
    }
}

/// Steps of a grid drawing whose run across is at most `GRID_JOG` cells long.
fn grid_jogs(d: &Drawing) -> usize {
    use canvas::Dir;
    let mut count = 0;
    for (path, out, _) in &d.routes {
        // The runs of the route: their heading and their length in cells.
        let mut runs: Vec<(Dir, usize)> = vec![(*out, 0)];
        for w in path.windows(2) {
            let dir = Dir::between(w[0], w[1]);
            match runs.last_mut() {
                Some(r) if r.0 == dir => r.1 += 1,
                _ => runs.push((dir, 1)),
            }
        }
        count += runs
            .windows(3)
            .filter(|w| w[0].0 == w[2].0 && w[1].1 <= GRID_JOG)
            .count();
    }
    count
}

/// The length in cells under which a step of a grid drawing counts as a jog.
const GRID_JOG: usize = 2;

/// Edges of a grid drawing whose last `GRID_TRUNK` cells are those of another edge too.
fn grid_merged(d: &Drawing) -> usize {
    let tail = |p: &[(usize, usize)]| p[p.len().saturating_sub(GRID_TRUNK)..].to_vec();
    d.routes
        .iter()
        .enumerate()
        .filter(|&(i, r)| {
            r.0.len() >= GRID_TRUNK
                && d.routes
                    .iter()
                    .enumerate()
                    .any(|(j, o)| i != j && o.0.len() >= GRID_TRUNK && tail(&o.0) == tail(&r.0))
        })
        .count()
}

/// The length in cells of a shared last run that makes 2 edges of a grid drawing merged.
const GRID_TRUNK: usize = 3;

/// Routes of a grid drawing with a step (a turn and a turn back), layers whose boxes differ in
/// length along the flow, and the largest distance in cells between a port and its even place on
/// its side.
fn grid_shape(d: &Drawing) -> (usize, usize, f64) {
    use canvas::Dir;
    let mut steps = 0;
    for (path, out, arrive) in &d.routes {
        let mut heads = vec![*out];
        for (i, &cell) in path.iter().enumerate() {
            let next = path.get(i + 1).map_or(*arrive, |&n| Dir::between(cell, n));
            if heads.last() != Some(&next) {
                heads.push(next);
            }
        }
        if heads.windows(3).any(|w| w[0] == w[2]) {
            steps += 1;
        }
    }
    let mut layers: std::collections::BTreeMap<usize, (usize, usize)> =
        std::collections::BTreeMap::new();
    for &(x, y, w, h, diamond) in &d.boxes {
        if diamond {
            continue;
        }
        let (key, along) = if d.horizontal {
            (2 * x + w, w)
        } else {
            (2 * y + h, h)
        };
        let e = layers.entry(key).or_insert((along, along));
        *e = (e.0.min(along), e.1.max(along));
    }
    let ragged = layers.values().filter(|l| l.0 != l.1).count();
    // Ports on each side of each node, the side as the direction out of the node.
    let mut sides: std::collections::BTreeMap<(usize, usize), Vec<usize>> =
        std::collections::BTreeMap::new();
    for ((path, out, arrive), &(from, to)) in d.routes.iter().zip(&d.links) {
        let (Some(&first), Some(&last)) = (path.first(), path.last()) else {
            continue;
        };
        let back = out.opposite();
        let source = back.step(first);
        let target = arrive.step(last);
        let along = |dir: Dir, c: (usize, usize)| {
            if matches!(dir, Dir::N | Dir::S) {
                c.0
            } else {
                c.1
            }
        };
        sides
            .entry((from, out.index()))
            .or_default()
            .push(along(*out, source));
        let side = arrive.opposite();
        sides
            .entry((to, side.index()))
            .or_default()
            .push(along(side, target));
    }
    let mut uneven: f64 = 0.0;
    for (&(node, side), ports) in &mut sides {
        let (x, y, w, h, _) = d.boxes[node];
        let (start, len) = if matches!(Dir::ALL[side], Dir::N | Dir::S) {
            (x, w)
        } else {
            (y, h)
        };
        ports.sort_unstable();
        ports.dedup();
        let n = ports.len();
        for (k, &p) in ports.iter().enumerate() {
            let ideal = start as f64 + (len - 1) as f64 * (k + 1) as f64 / (n + 1) as f64;
            uneven = uneven.max((p as f64 - ideal).abs());
        }
    }
    (steps, ragged, uneven)
}

/// Crossings and turns of the routes of a grid drawing.
fn grid_measure(routes: &[Route]) -> (usize, usize) {
    let mut tally = Tally::default();
    for route in routes {
        tally.add(route);
    }
    (tally.crossings, tally.bends)
}

/// The crossings and the turns of the routes of a grid drawing, counted as routes come.
#[derive(Default)]
struct Tally {
    /// Each cell a route runs straight through: the route and whether it runs along a column.
    cells: std::collections::HashMap<(usize, usize), Vec<(usize, bool)>>,
    routes: usize,
    crossings: usize,
    bends: usize,
}

impl Tally {
    /// Counts `route`: its turns, and the cells where it runs across a route counted before.
    fn add(&mut self, (path, out, arrive): &Route) {
        use canvas::Dir;
        let e = self.routes;
        self.routes += 1;
        let mut heading = *out;
        for (i, &cell) in path.iter().enumerate() {
            let next = path.get(i + 1).map_or(*arrive, |&n| Dir::between(cell, n));
            if next != heading {
                self.bends += 1;
            }
            let vertical = |d: Dir| matches!(d, Dir::N | Dir::S);
            if vertical(heading) == vertical(next) {
                let list = self.cells.entry(cell).or_default();
                let along = vertical(next);
                self.crossings += list.iter().filter(|b| b.0 != e && b.1 != along).count();
                list.push((e, along));
            }
            heading = next;
        }
    }
}

/// Draws `code` as an SVG element for a page. `None` when the diagram type or syntax is not drawn.
/// A pie chart is a pie. Another diagram is drawn as vector graphics, or with the layout it has
/// in the terminal when the vector drawing does not handle it.
pub fn svg(code: &str) -> Option<String> {
    let first = code
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with("%%"))?;
    if first.split_whitespace().next() == Some("pie") {
        return pie::svg(code);
    }
    first_drawn(code, vector::svg)
}

/// The drawing of `vector`, or the one with the layout of the terminal when `vector` has none.
fn first_drawn(code: &str, vector: fn(&str) -> Option<String>) -> Option<String> {
    vector(code).or_else(|| grid_svg(code))
}

/// Draws `code` as an SVG element with the layout of the terminal.
fn grid_svg(code: &str) -> Option<String> {
    let opts = RenderOptions {
        glyphs: Glyphs::unicode(),
        ..RenderOptions::plain(PAGE_WIDTH)
    };
    match draw(code, PAGE_WIDTH, &opts)? {
        Drawn::Grid(drawing) => Some(drawing.canvas.to_svg(code)),
        Drawn::Chart(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flowchart_is_an_svg_with_its_labels_and_arrow_heads() {
        let svg = svg("graph LR\n  A[Write] --> B[Check]\n").unwrap();
        assert!(svg.starts_with("<svg "), "{svg}");
        assert!(svg.contains("Write") && svg.contains("Check"), "{svg}");
        assert!(svg.contains("<path d=\"M"), "{svg}");
        assert!(svg.contains("<desc>graph LR"), "{svg}");
    }

    #[test]
    fn a_page_gets_the_vector_drawing() {
        let svg = svg("graph TD\n  A --> B\n").unwrap();
        assert!(
            svg.contains("sans-serif") && svg.contains("max-width:100%"),
            "{svg}"
        );
    }

    #[test]
    fn the_grid_drawing_is_the_fallback() {
        let code = "graph LR\n  A[Write] --> B[Check]\n";
        let grid = first_drawn(code, |_| None).unwrap();
        assert!(grid.contains("monospace"), "{grid}");
        assert_eq!(Some(grid), grid_svg(code));
        assert!(first_drawn("gantt\n  title A\n", |_| None).is_none());
    }

    #[test]
    fn what_is_not_drawn_has_no_svg() {
        assert!(svg("gantt\n  title A\n").is_none());
        assert!(svg("graph LR\n  A -->").is_none() || svg("not a diagram").is_none());
    }

    #[test]
    fn degenerate_graphs_are_drawn_or_refused_without_a_panic() {
        // A lone node, self-loops, repeated edges, 2-cycles, isolated nodes, a wide fan, an
        // edge to an undeclared node, and a long cycle.
        let fan: String = (0..14).map(|k| format!(" H --> F{k}\n")).collect();
        let ring: String = (0..30)
            .map(|k| format!(" R{k} --> R{}\n", (k + 1) % 30))
            .collect();
        let sources = [
            "graph TD\n A\n".to_string(),
            "graph TD\n A --> A\n A --> A\n".to_string(),
            "graph LR\n A --> B\n A --> B\n A -->|x| B\n B --> A\n".to_string(),
            "graph TD\n A --> B\n C\n D\n B --> A\n E --> E\n".to_string(),
            format!("graph TD\n{fan} F3 --> H\n F3 --> F3\n"),
            format!("graph LR\n{ring}"),
            "graph TD\n subgraph s [S]\n A --> A\n end\n A --> B\n B --> A\n".to_string(),
        ];
        let opts = RenderOptions {
            glyphs: Glyphs::unicode(),
            ..RenderOptions::plain(400)
        };
        for src in &sources {
            let _ = svg(src);
            let _ = render(src, 400, &opts);
            let _ = render(src, 20, &opts);
            let _ = measure(src);
        }
    }
}

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

use canvas::Canvas;

use crate::model::Line;
use crate::options::{Glyphs, RenderOptions};

/// A diagram as the lines of the terminal, and the grid that they were drawn from.
pub struct Drawing {
    pub lines: Vec<Line>,
    pub canvas: Canvas,
}

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

/// Draws `code` as an SVG element for a page. `None` when the diagram type or syntax is not drawn.
/// A pie chart is a pie, and every other diagram has the layout it has in the terminal.
pub fn svg(code: &str) -> Option<String> {
    let first = code
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with("%%"))?;
    if first.split_whitespace().next() == Some("pie") {
        return pie::svg(code);
    }
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
    fn what_is_not_drawn_has_no_svg() {
        assert!(svg("gantt\n  title A\n").is_none());
        assert!(svg("graph LR\n  A -->").is_none() || svg("not a diagram").is_none());
    }
}

//! Pie charts drawn as horizontal bars.

use crate::model::{Line, Segment};
use crate::options::RenderOptions;
use crate::wrap::display_width;

/// Blank cells between the label, the bar and the figures.
const COLUMN_GAP: usize = 2;
/// Cells under which the bars are not worth drawing.
const MIN_BAR: usize = 8;

#[derive(Debug, PartialEq)]
struct Pie {
    title: Option<String>,
    show_data: bool,
    slices: Vec<(String, f64)>,
}

/// Parses a `pie` block. `None` for another diagram type, a malformed slice or no positive total.
fn parse(src: &str) -> Option<Pie> {
    let mut lines = src
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("%%"));
    let mut header = lines.next()?.split_whitespace();
    if header.next()? != "pie" {
        return None;
    }
    let mut pie = Pie {
        title: None,
        show_data: false,
        slices: Vec::new(),
    };
    let mut rest: Vec<&str> = header.collect();
    if rest.first() == Some(&"showData") {
        pie.show_data = true;
        rest.remove(0);
    }
    match rest.split_first() {
        Some((&"title", words)) => pie.title = Some(words.join(" ")),
        Some(_) => return None,
        None => {}
    }
    for line in lines {
        if let Some(title) = line.strip_prefix("title ") {
            pie.title = Some(title.trim().to_string());
            continue;
        }
        let quoted = line.strip_prefix('"')?;
        let (label, after) = quoted.split_once('"')?;
        let value: f64 = after.trim().strip_prefix(':')?.trim().parse().ok()?;
        if !value.is_finite() || value < 0.0 {
            return None;
        }
        pie.slices.push((label.to_string(), value));
    }
    let total: f64 = pie.slices.iter().map(|s| s.1).sum();
    (total > 0.0).then_some(pie)
}

fn draw(pie: &Pie, width: usize, opts: &RenderOptions) -> Option<Vec<Line>> {
    let total: f64 = pie.slices.iter().map(|s| s.1).sum();
    let largest = pie.slices.iter().map(|s| s.1).fold(0.0, f64::max);
    let label_width = pie
        .slices
        .iter()
        .map(|s| display_width(&s.0))
        .max()
        .unwrap_or(0);
    let figures: Vec<String> = pie
        .slices
        .iter()
        .map(|(_, v)| {
            let share = format!("{:.1}%", v / total * 100.0);
            if pie.show_data {
                format!("{share} ({v})")
            } else {
                share
            }
        })
        .collect();
    let figure_width = figures.iter().map(|f| display_width(f)).max().unwrap_or(0);
    let bar_max = width.checked_sub(label_width + figure_width + 2 * COLUMN_GAP)?;
    if bar_max < MIN_BAR {
        return None;
    }
    let mut out: Vec<Line> = Vec::new();
    if let Some(title) = &pie.title {
        out.push(vec![Segment::new(title.clone(), opts.theme.base.bold())]);
    }
    for ((label, value), figure) in pie.slices.iter().zip(&figures) {
        let cells =
            ((value / largest * bar_max as f64).round() as usize).max(usize::from(*value > 0.0));
        let pad = " ".repeat(label_width - display_width(label) + COLUMN_GAP);
        let bar: String = std::iter::repeat_n(opts.glyphs.bar, cells).collect();
        let fill = " ".repeat(bar_max - cells + COLUMN_GAP);
        out.push(vec![
            Segment::new(format!("{label}{pad}"), opts.theme.base),
            Segment::new(bar, opts.theme.table_border),
            Segment::new(format!("{fill}{figure}"), opts.theme.base),
        ]);
    }
    Some(out)
}

/// The colours of the slices, which read on a light and on a dark page.
const SLICES: [&str; 8] = [
    "#4e79a7", "#f28e2b", "#59a14f", "#e15759", "#b07aa1", "#76b7b2", "#edc948", "#9c755f",
];

/// A `pie` block as an SVG element: the pie, and a legend with the share of each slice.
pub fn svg(code: &str) -> Option<String> {
    let pie = parse(code)?;
    let total: f64 = pie.slices.iter().map(|s| s.1).sum();
    let (radius, cx, cy) = (90.0_f64, 100.0_f64, 100.0_f64);
    let title_height = if pie.title.is_some() { 28.0 } else { 0.0 };
    let mut shapes = String::new();
    let mut angle = -std::f64::consts::FRAC_PI_2;
    let point = |a: f64| (cx + radius * a.cos(), cy + title_height + radius * a.sin());
    for (i, (_, value)) in pie.slices.iter().enumerate() {
        let sweep = value / total * std::f64::consts::TAU;
        let color = SLICES[i % SLICES.len()];
        if pie.slices.len() == 1 {
            shapes.push_str(&format!(
                "<circle cx=\"{cx}\" cy=\"{}\" r=\"{radius}\" fill=\"{color}\"/>",
                cy + title_height
            ));
        } else if *value > 0.0 {
            let (x1, y1) = point(angle);
            let (x2, y2) = point(angle + sweep);
            let large = u8::from(sweep > std::f64::consts::PI);
            shapes.push_str(&format!(
                "<path d=\"M{cx} {}L{x1:.2} {y1:.2}A{radius} {radius} 0 {large} 1 {x2:.2} {y2:.2}Z\" fill=\"{color}\" stroke=\"var(--bg, #fff)\" stroke-width=\"1.5\"/>",
                cy + title_height
            ));
        }
        angle += sweep;
    }
    let mut legend = String::new();
    for (i, (label, value)) in pie.slices.iter().enumerate() {
        let y = title_height + 24.0 + i as f64 * 22.0;
        let share = format!("{:.1}%", value / total * 100.0);
        let figure = if pie.show_data {
            format!("{share} ({value})")
        } else {
            share
        };
        legend.push_str(&format!(
            "<rect x=\"220\" y=\"{:.1}\" width=\"12\" height=\"12\" rx=\"2\" fill=\"{}\"/><text x=\"240\" y=\"{:.1}\">{} <tspan opacity=\".7\">{}</tspan></text>",
            y - 10.0,
            SLICES[i % SLICES.len()],
            y,
            crate::escape::html(label),
            crate::escape::html(&figure)
        ));
    }
    let height = (title_height + 2.0 * radius + 20.0)
        .max(title_height + 30.0 + pie.slices.len() as f64 * 22.0);
    let title = pie
        .title
        .as_ref()
        .map(|t| {
            format!(
                "<text x=\"{cx}\" y=\"18\" text-anchor=\"middle\" font-weight=\"600\">{}</text>",
                crate::escape::html(t)
            )
        })
        .unwrap_or_default();
    Some(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 520 {height}\" width=\"520\" height=\"{height}\" role=\"img\" aria-label=\"Pie chart\" font-family=\"system-ui, sans-serif\" font-size=\"13\" fill=\"currentColor\"><title>Pie chart</title><desc>{}</desc>{title}{shapes}{legend}</svg>",
        crate::escape::html(code.trim_end())
    ))
}

/// Draws a `pie` block within `width` columns.
pub fn render(code: &str, width: usize, opts: &RenderOptions) -> Option<Vec<Line>> {
    draw(&parse(code)?, width, opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draw_plain(src: &str, width: usize) -> Option<String> {
        let lines = render(src, width, &RenderOptions::plain(width))?;
        Some(
            lines
                .iter()
                .map(|l| l.iter().map(|s| s.text.as_str()).collect::<String>() + "\n")
                .collect(),
        )
    }

    #[test]
    fn parses_title_data_and_slices() {
        let pie = parse("pie showData title Pets\n \"Dogs\" : 386\n \"Cats\" : 85.5\n").unwrap();
        assert!(pie.show_data);
        assert_eq!(pie.title.as_deref(), Some("Pets"));
        assert_eq!(pie.slices, [("Dogs".into(), 386.0), ("Cats".into(), 85.5)]);
        assert!(parse("pie\n \"a\" : x").is_none());
        assert!(parse("pie\n \"a\" : 0").is_none());
        assert!(parse("graph TD\n A --> B").is_none());
    }

    #[test]
    fn bars_are_scaled_to_the_largest_slice() {
        let art = draw_plain("pie title Pets\n \"Dogs\" : 75\n \"Cats\" : 25\n", 30).unwrap();
        assert_eq!(
            art,
            "Pets\nDogs  #################  75.0%\nCats  ######             25.0%\n"
        );
    }

    #[test]
    fn narrow_width_falls_back() {
        assert!(draw_plain("pie\n \"a\" : 1\n \"b\" : 2\n", 12).is_none());
    }
}

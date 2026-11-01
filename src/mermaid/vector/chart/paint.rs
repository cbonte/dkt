//! The SVG of the parts of a drawing: edge paths, heads, node shapes and node text.

use super::*;

pub(super) fn path(segs: &[Seg], dx: f64, dy: f64) -> String {
    let mut d = String::new();
    let pt = |p: P| format!("{} {}", num(p.0 + dx), num(p.1 + dy));
    for s in segs {
        match *s {
            Seg::Move(p) => d.push_str(&format!("M{}", pt(p))),
            Seg::Line(p) => d.push_str(&format!("L{}", pt(p))),
            Seg::Curve(a, b, p) => d.push_str(&format!("C{} {} {}", pt(a), pt(b), pt(p))),
        }
    }
    d
}

/// The marks of `head` at the tip `e` of an edge heading `d`.
pub(super) fn head_marks(head: Head, e: P, d: P) -> String {
    let n = (-d.1, d.0);
    let at = |along: f64, side: f64| {
        (
            e.0 - d.0 * along + n.0 * side,
            e.1 - d.1 * along + n.1 * side,
        )
    };
    let poly = |pts: &[P], fill: bool| {
        let mut s = String::from("<path d=\"");
        for (i, p) in pts.iter().enumerate() {
            s.push_str(&format!(
                "{}{} {}",
                if i == 0 { "M" } else { "L" },
                num(p.0),
                num(p.1)
            ));
        }
        s.push_str("Z\"");
        if fill {
            s.push_str(" fill=\"currentColor\"");
        }
        s.push_str("/>");
        s
    };
    let line = |a: P, b: P| {
        format!(
            "<path d=\"M{} {}L{} {}\"/>",
            num(a.0),
            num(a.1),
            num(b.0),
            num(b.1)
        )
    };
    let circle = |c: P, r: f64| {
        format!(
            "<circle cx=\"{}\" cy=\"{}\" r=\"{}\"/>",
            num(c.0),
            num(c.1),
            num(r)
        )
    };
    let bar = |along: f64| line(at(along, -6.0), at(along, 6.0));
    let crow = || {
        format!(
            "{}{}",
            line(at(0.0, -6.0), at(10.0, 0.0)),
            line(at(0.0, 6.0), at(10.0, 0.0))
        )
    };
    match head {
        Head::None => String::new(),
        Head::Arrow => poly(&[e, at(9.0, -4.5), at(9.0, 4.5)], true),
        Head::Circle => circle(at(4.5, 0.0), 4.0),
        Head::Cross => format!(
            "{}{}",
            line(at(1.0, -4.0), at(9.0, 4.0)),
            line(at(1.0, 4.0), at(9.0, -4.0))
        ),
        Head::Inherit => poly(&[e, at(13.0, -7.0), at(13.0, 7.0)], false),
        Head::Composition | Head::Aggregation => poly(
            &[e, at(8.5, -5.5), at(17.0, 0.0), at(8.5, 5.5)],
            head == Head::Composition,
        ),
        Head::One => format!("{}{}", bar(6.0), bar(11.0)),
        Head::ZeroOne => format!(
            "{}{}{}",
            line(e, at(10.0, 0.0)),
            bar(6.0),
            circle(at(14.0, 0.0), 4.0)
        ),
        Head::OneMany => format!("{}{}", crow(), bar(14.0)),
        Head::ZeroMany => format!(
            "{}{}{}",
            line(e, at(11.0, 0.0)),
            crow(),
            circle(at(15.0, 0.0), 4.0)
        ),
    }
}

/// The outline of a node centred on `c`.
/// The colour class of a node by its role.
pub(super) fn role(shape: Shape) -> &'static str {
    match shape.role() {
        Role::End => "dkt-terminal",
        Role::Decision => "dkt-decision",
        Role::Store => "dkt-store",
        Role::Step => "dkt-step",
    }
}

pub(super) fn shape_svg(s: &Shaped, c: P, dx: f64, dy: f64, kind: Kind) -> String {
    let (cx, cy) = (c.0 + dx, c.1 + dy);
    let (x0, y0, x1, y1) = (
        cx - s.w / 2.0,
        cy - s.h / 2.0,
        cx + s.w / 2.0,
        cy + s.h / 2.0,
    );
    let poly = |pts: &[P]| {
        let points: Vec<String> = pts
            .iter()
            .map(|p| format!("{},{}", num(p.0), num(p.1)))
            .collect();
        format!("<polygon points=\"{}\"/>", points.join(" "))
    };
    let rect = |rx: f64| {
        format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\"/>",
            num(x0),
            num(y0),
            num(s.w),
            num(s.h),
            num(rx)
        )
    };
    let circle = |r: f64| {
        format!(
            "<circle cx=\"{}\" cy=\"{}\" r=\"{}\"/>",
            num(cx),
            num(cy),
            num(r)
        )
    };
    let inset = s.h * 0.35;
    match s.shape {
        Shape::Rect => rect(3.0),
        Shape::Round => rect(if kind == Kind::State { 10.0 } else { 9.0 }),
        Shape::Stadium => rect(s.h.min(s.w) / 2.0),
        Shape::Circle => circle(s.w / 2.0),
        Shape::DoubleCircle => format!("{}{}", circle(s.w / 2.0), circle(s.w / 2.0 - 4.0)),
        Shape::Diamond => poly(&[(cx, y0), (x1, cy), (cx, y1), (x0, cy)]),
        Shape::Hexagon => {
            let i = s.h / 4.0;
            poly(&[
                (x0 + i, y0),
                (x1 - i, y0),
                (x1, cy),
                (x1 - i, y1),
                (x0 + i, y1),
                (x0, cy),
            ])
        }
        Shape::Asymmetric => poly(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0 + s.h / 3.0, cy)]),
        Shape::Lean(true) => poly(&[(x0 + inset, y0), (x1, y0), (x1 - inset, y1), (x0, y1)]),
        Shape::Lean(false) => poly(&[(x0, y0), (x1 - inset, y0), (x1, y1), (x0 + inset, y1)]),
        Shape::Trapezoid(true) => poly(&[(x0 + inset, y0), (x1 - inset, y0), (x1, y1), (x0, y1)]),
        Shape::Trapezoid(false) => poly(&[(x0, y0), (x1, y0), (x1 - inset, y1), (x0 + inset, y1)]),
        Shape::Subroutine => format!(
            "{}<path d=\"M{} {}V{}M{} {}V{}\" fill=\"none\"/>",
            rect(0.0),
            num(x0 + 7.0),
            num(y0),
            num(y1),
            num(x1 - 7.0),
            num(y0),
            num(y1)
        ),
        Shape::Cylinder => {
            let (rx, ry) = (s.w / 2.0, cylinder_ry(s.w));
            format!(
                "<path d=\"M{x0} {t}A{rx} {ry} 0 0 1 {x1} {t}V{b}A{rx} {ry} 0 0 1 {x0} {b}Z\"/><path d=\"M{x0} {t}A{rx} {ry} 0 0 0 {x1} {t}\" fill=\"none\"/>",
                x0 = num(x0),
                x1 = num(x1),
                t = num(y0 + ry),
                b = num(y1 - ry),
                rx = num(rx),
                ry = num(ry)
            )
        }
        Shape::Start => format!(
            "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"currentColor\" fill-opacity=\"1\"/>",
            num(cx),
            num(cy),
            num(s.w / 2.0)
        ),
        Shape::End => format!(
            "{}<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"currentColor\" fill-opacity=\"1\"/>",
            circle(s.w / 2.0),
            num(cx),
            num(cy),
            num(s.w / 2.0 - 4.0)
        ),
    }
}

/// The text of a node centred on `c`, and the lines between its compartments.
pub(super) fn node_text(s: &Shaped, c: P, dx: f64, dy: f64) -> (String, String) {
    let (cx, cy) = (c.0 + dx, c.1 + dy);
    if s.parts.is_empty() {
        return (String::new(), String::new());
    }
    let parts_h: f64 = s.parts.iter().map(Part::height).sum();
    let total = parts_h + (s.parts.len() - 1) as f64 * 2.0 * s.pad_y;
    let extra = if s.shape == Shape::Cylinder {
        cylinder_ry(s.w)
    } else {
        0.0
    };
    let mut y = cy - total / 2.0 + extra;
    let (mut words, mut rules) = (String::new(), String::new());
    let left = cx - s.w / 2.0 + s.pad_x;
    for (i, part) in s.parts.iter().enumerate() {
        if i > 0 {
            let ry = y - s.pad_y;
            rules.push_str(&format!(
                "M{} {}H{}",
                num(cx - s.w / 2.0),
                num(ry),
                num(cx + s.w / 2.0)
            ));
        }
        let step = SIZE * LEADING;
        for (r, (cells, look)) in part.rows.iter().enumerate() {
            let row_cy = y + step * (r as f64 + 0.5);
            let extra = match look {
                Look::Bold => " font-weight=\"bold\"",
                Look::Italic => " font-style=\"italic\"",
                Look::Regular => "",
            };
            if part.centred {
                let line = cells.join(" ");
                words.push_str(&text::text(
                    &[line],
                    cx,
                    row_cy,
                    SIZE,
                    Anchor::Middle,
                    extra,
                ));
            } else {
                let mut x = left;
                for (c, cell) in cells.iter().enumerate() {
                    words.push_str(&text::text(
                        std::slice::from_ref(cell),
                        x,
                        row_cy,
                        SIZE,
                        Anchor::Start,
                        extra,
                    ));
                    x += part.columns.get(c).copied().unwrap_or(0.0) + COLUMN_GAP;
                }
            }
        }
        y += part.height() + 2.0 * s.pad_y;
    }
    (words, rules)
}

//! Mermaid diagrams drawn as vector graphics for a page: boxes sized by their text, a layered
//! layout, edges that go around the nodes, proportional text and a size that fits a column.

mod chart;
pub(super) mod layered;
mod sequence;
mod text;

use chart::Kind;

/// Draws `code` as an SVG element. `None` when the diagram type or its syntax is not drawn, or
/// when the layout fails.
pub fn svg(code: &str) -> Option<String> {
    let first = code
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with("%%"))?;
    let word = first.trim_end_matches(';').split_whitespace().next()?;
    let (flow, kind) = match word {
        "sequenceDiagram" => return sequence::svg(code),
        "graph" | "flowchart" => (super::parse::parse(code)?, Kind::Flowchart),
        "stateDiagram" | "stateDiagram-v2" => (super::state::parse(code)?, Kind::State),
        "classDiagram" | "classDiagram-v2" => (super::class::parse(code)?, Kind::Class),
        "erDiagram" => (super::er::parse(code)?, Kind::Entity),
        _ => return None,
    };
    chart::svg(&flow, kind, code)
}

/// The crossings, the bends and the length of the edges of the vector drawing of `code`. `None` for a diagram
/// that is not a graph or is not drawn.
pub fn measure(code: &str) -> Option<chart::Counted> {
    let first = code
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with("%%"))?;
    let word = first.trim_end_matches(';').split_whitespace().next()?;
    let (flow, kind) = match word {
        "graph" | "flowchart" => (super::parse::parse(code)?, Kind::Flowchart),
        "stateDiagram" | "stateDiagram-v2" => (super::state::parse(code)?, Kind::State),
        "classDiagram" | "classDiagram-v2" => (super::class::parse(code)?, Kind::Class),
        "erDiagram" => (super::er::parse(code)?, Kind::Entity),
        _ => return None,
    };
    chart::measure(&flow, kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The number in the attribute `name` of the `<svg>` element.
    fn attribute(svg: &str, name: &str) -> f64 {
        let head = &svg[..svg.find('>').unwrap()];
        let at = head.find(&format!(" {name}=\"")).unwrap() + name.len() + 3;
        head[at..].split('"').next().unwrap().parse().unwrap()
    }

    #[test]
    fn a_subgraph_is_a_titled_box() {
        let svg = svg("graph TD\n subgraph s [Back end]\n A --> B\n end\n C --> A\n").unwrap();
        assert!(svg.contains(">Back end<"), "{svg}");
        assert!(svg.contains("rx=\"6\""), "{svg}");
    }

    #[test]
    fn an_edge_label_is_text_that_cuts_its_edge() {
        let svg = svg("graph TD\n A -->|yes| B\n").unwrap();
        assert!(svg.contains(">yes</text>"), "{svg}");
        let edges = svg.split("stroke-linecap").next().unwrap();
        let path = &edges[edges.rfind("<path d=\"").unwrap()..];
        assert_eq!(path.matches('M').count(), 2, "{path}");
    }

    #[test]
    fn the_same_source_gives_the_same_bytes() {
        let src =
            "flowchart LR\n A --> B & C\n subgraph g [G]\n B --> D\n end\n C --> D\n D --> A\n";
        assert_eq!(svg(src), svg(src));
        let seq = "sequenceDiagram\n A->>B: x\n loop l\n B->>B: y\n end\n";
        assert_eq!(svg(seq), svg(seq));
    }

    #[test]
    fn a_row_of_8_boxes_stays_left_to_right() {
        let chain: String = (0..7)
            .map(|i| {
                format!(
                    " N{i}[Step number {i}] --> N{}[Step number {}]\n",
                    i + 1,
                    i + 1
                )
            })
            .collect();
        let svg = svg(&format!("graph LR\n{chain}")).unwrap();
        let (w, h) = (attribute(&svg, "width"), attribute(&svg, "height"));
        assert!(w > 3.0 * h, "{w} by {h}");
        // The compact styles make it narrower than the roomy one would be.
        assert!(w < 8.0 * 140.0, "{w}");
        assert!(svg.contains("viewBox=\"0 0 "));
    }

    #[test]
    fn a_top_down_flow_stays_top_down() {
        let svg = svg("graph TD\n A --> B\n B --> C\n").unwrap();
        assert!(attribute(&svg, "height") > attribute(&svg, "width"));
    }

    #[test]
    fn state_class_and_entity_diagrams_are_drawn() {
        let state = svg("stateDiagram-v2\n [*] --> A\n A --> [*]\n").unwrap();
        assert!(state.contains("<circle"), "{state}");
        let class = svg("classDiagram\n class A {\n +x int\n +f() void\n }\n A <|-- B\n").unwrap();
        assert!(
            class.contains("+f() void") && class.contains("font-weight=\"bold\""),
            "{class}"
        );
        let er = svg("erDiagram\n A ||--o{ B : has\n A {\n int id PK\n }\n").unwrap();
        assert!(er.contains(">PK<") && er.contains(">has<"), "{er}");
    }

    #[test]
    fn parts_take_the_colour_of_their_role() {
        let svg = svg("graph TD\n A([Start]) --> B{Ready?}\n B -->|yes| C[(Store)]\n B --> D[Step]\n D --> E[Next]\n").unwrap();
        assert!(
            svg.contains("<style>") && svg.contains("--dkt-diagram-step"),
            "{svg}"
        );
        for class in ["dkt-terminal", "dkt-decision", "dkt-store", "dkt-step"] {
            assert!(svg.contains(&format!("dkt-fill {class}")), "{class}: {svg}");
        }
        // The main path and the label pills take the accent.
        assert!(svg.contains("class=\"dkt-line dkt-accent\""), "{svg}");
        assert!(svg.contains("class=\"dkt-pill dkt-accent\""), "{svg}");
    }

    #[test]
    fn other_diagrams_are_left_to_the_grid() {
        assert!(svg("gantt\n title A\n").is_none());
        assert!(svg("graph TD\n A -->").is_none());
    }
}

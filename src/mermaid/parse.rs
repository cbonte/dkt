//! Flowchart syntax: header, nodes, edges and subgraphs.

use std::collections::HashMap;

use super::canvas::Weight;
use super::graph::{Builder, Direction, Edge, Flow, Group, Head, Node, Shape};

/// Opening and closing delimiters of the node shapes, longest opening first.
const SHAPES: [(&str, &str, Shape); 14] = [
    ("(((", ")))", Shape::DoubleCircle),
    ("((", "))", Shape::Circle),
    ("([", "])", Shape::Stadium),
    ("[[", "]]", Shape::Subroutine),
    ("[(", ")]", Shape::Cylinder),
    ("{{", "}}", Shape::Hexagon),
    ("[/", "/]", Shape::Lean(true)),
    ("[\\", "\\]", Shape::Lean(false)),
    ("[/", "\\]", Shape::Trapezoid(true)),
    ("[\\", "/]", Shape::Trapezoid(false)),
    ("[", "]", Shape::Rect),
    ("(", ")", Shape::Round),
    ("{", "}", Shape::Diamond),
    (">", "]", Shape::Asymmetric),
];

/// Subgraphs being read, innermost last, each with the nodes named inside it.
struct Open {
    group: usize,
    named: Vec<usize>,
}

/// Parses a `graph` or `flowchart` block. `None` for another diagram type and for syntax that is
/// not drawn.
pub fn parse(src: &str) -> Option<Flow> {
    let mut lines = src
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("%%"));
    let mut header = lines.next()?.trim_end_matches(';').split_whitespace();
    if !matches!(header.next()?, "graph" | "flowchart") {
        return None;
    }
    let direction = match header.next() {
        Some(token) => Direction::parse(token)?,
        None => Direction::TopDown,
    };
    if header.next().is_some() {
        return None;
    }
    let mut builder = Builder::new(direction);
    let mut open: Vec<Open> = Vec::new();
    let mut group_ids: HashMap<String, usize> = HashMap::new();
    for line in lines {
        for statement in line.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            let (word, rest) = statement
                .split_once(char::is_whitespace)
                .unwrap_or((statement, ""));
            match word {
                "style" | "classDef" | "class" | "linkStyle" | "click" | "accTitle"
                | "accDescr" | "direction" => continue,
                "subgraph" => {
                    let (id, title) = subgraph_header(rest)?;
                    if builder.has(&id) || group_ids.contains_key(&id) {
                        return None;
                    }
                    let group = builder.flow.groups.len();
                    builder.flow.groups.push(Group {
                        title,
                        parent: open.last().map(|o| o.group),
                        nodes: Vec::new(),
                    });
                    group_ids.insert(id, group);
                    open.push(Open {
                        group,
                        named: Vec::new(),
                    });
                    continue;
                }
                "end" if rest.is_empty() => {
                    let closed = open.pop()?;
                    close(&mut builder.flow, &closed);
                    continue;
                }
                _ => {}
            }
            if word.starts_with("accTitle:") || word.starts_with("accDescr:") {
                continue;
            }
            builder.named.clear();
            parse_statement(statement, &mut builder, &group_ids)?;
            if let Some(innermost) = open.last_mut() {
                innermost.named.append(&mut builder.named);
            }
        }
    }
    open.is_empty().then_some(builder.flow)
}

/// The nodes named inside a subgraph that no subgraph inside it holds become its own.
fn close(flow: &mut Flow, closed: &Open) {
    for &node in &closed.named {
        if !flow.groups.iter().any(|g| g.nodes.contains(&node)) {
            flow.groups[closed.group].nodes.push(node);
        }
    }
}

/// The name and the title of `subgraph id [title]`, `subgraph "title"` or `subgraph title`.
fn subgraph_header(rest: &str) -> Option<(String, Vec<String>)> {
    let rest = rest.trim();
    if rest.is_empty() {
        return None;
    }
    if let Some(open) = rest.find('[') {
        let id = rest[..open].trim();
        let (title, after) = parse_label(&rest[open + 1..], "]")?;
        if id.is_empty() || id.contains(char::is_whitespace) || !after.trim().is_empty() {
            return None;
        }
        let title = if title.is_empty() {
            vec![id.to_string()]
        } else {
            title
        };
        return Some((id.to_string(), title));
    }
    let text = rest
        .strip_prefix('"')
        .and_then(|r| r.strip_suffix('"'))
        .unwrap_or(rest);
    Some((text.to_string(), label_lines(text)))
}

/// One chain such as `A[Start] --> B & C --> D`.
fn parse_statement(s: &str, b: &mut Builder, groups: &HashMap<String, usize>) -> Option<()> {
    let mut rest = s;
    let mut pending: Option<(Vec<usize>, Link)> = None;
    loop {
        let mut nodes = Vec::new();
        loop {
            let (node, after) = parse_node(rest, b, groups)?;
            nodes.push(node);
            rest = after.trim_start();
            match rest.strip_prefix('&') {
                Some(after) => rest = after.trim_start(),
                None => break,
            }
        }
        if let Some((sources, link)) = pending.take()
            && link.visible
        {
            for &from in &sources {
                for &to in &nodes {
                    b.flow.edges.push(Edge {
                        from,
                        to,
                        from_end: link.start,
                        to_end: link.end,
                        weight: link.weight,
                        label: link.label.clone(),
                    });
                }
            }
        }
        if rest.is_empty() {
            return Some(());
        }
        let (link, after) = parse_edge(rest)?;
        pending = Some((nodes, link));
        rest = after.trim_start();
    }
}

fn parse_node<'a>(
    s: &'a str,
    b: &mut Builder,
    groups: &HashMap<String, usize>,
) -> Option<(usize, &'a str)> {
    let (id, mut rest) = s.split_at(id_end(s));
    if id.is_empty() || groups.contains_key(id) {
        return None;
    }
    let mut shaped = None;
    for (open, close, shape) in SHAPES {
        if let Some(inner) = rest.strip_prefix(open)
            && let Some((label, after)) = parse_label(inner, close)
        {
            shaped = Some((label, shape));
            rest = after;
            break;
        }
    }
    if rest.starts_with('[') || rest.starts_with('(') || rest.starts_with('{') {
        return None;
    }
    if let Some(after) = rest.strip_prefix(":::") {
        rest = &after[id_end(after)..];
    }
    let index = b.node(id);
    if let Some((lines, shape)) = shaped.filter(|(lines, _)| !lines.is_empty()) {
        b.flow.nodes[index] = Node {
            lines,
            dividers: Vec::new(),
            shape,
        };
    }
    Some((index, rest))
}

/// Length of the node name at the start of `s`: letters, digits and underscores, and dashes that
/// do not start a link.
fn id_end(s: &str) -> usize {
    let bytes = s.as_bytes();
    let mut end = 0;
    for (i, c) in s.char_indices() {
        if c.is_alphanumeric() || c == '_' {
            end = i + c.len_utf8();
        } else if c == '-' && i > 0 && bytes.get(i + 1).is_some_and(|n| n.is_ascii_alphanumeric()) {
            end = i + 1;
        } else {
            break;
        }
    }
    end
}

/// Splits `s` at the delimiter `close`, ignoring one inside double quotes. Returns the label as
/// lines, `<br>` starting a new one, and the text after the delimiter.
fn parse_label<'a>(s: &'a str, close: &str) -> Option<(Vec<String>, &'a str)> {
    let s = s.trim_start();
    let (text, after) = if let Some(quoted) = s.strip_prefix('"') {
        let end = quoted.find('"')?;
        let after = quoted[end + 1..].trim_start().strip_prefix(close)?;
        (&quoted[..end], after)
    } else {
        let end = s.find(close)?;
        (&s[..end], &s[end + close.len()..])
    };
    Some((label_lines(text), after))
}

/// The lines of a label, `<br>` starting a new one.
fn label_lines(text: &str) -> Vec<String> {
    let text = text
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("<br>", "\n");
    text.lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

struct Link {
    start: Head,
    end: Head,
    weight: Weight,
    label: Option<String>,
    /// `~~~` places its nodes and draws nothing.
    visible: bool,
}

/// The head that a character before or after the line of a link stands for.
fn head_of(c: Option<char>, before: bool) -> Option<Head> {
    match c? {
        '<' if before => Some(Head::Arrow),
        '>' if !before => Some(Head::Arrow),
        'o' => Some(Head::Circle),
        'x' => Some(Head::Cross),
        _ => None,
    }
}

/// The weight of a line drawn with `chars`: `=` is thick, a dot is dotted.
fn weight_of(chars: &str) -> Weight {
    if chars.contains('=') {
        Weight::Thick
    } else if chars.contains('.') {
        Weight::Dotted
    } else {
        Weight::Normal
    }
}

/// Length of the run of line characters at the start of `s`.
fn line_run(s: &str) -> usize {
    s.find(|c: char| !matches!(c, '-' | '.' | '='))
        .unwrap_or(s.len())
}

/// Whether `run` draws a line: 2 or more dashes or equal signs, or dashes around dots.
fn is_line(run: &str) -> bool {
    let dashes = run.chars().filter(|&c| c == '-').count();
    let equals = run.chars().filter(|&c| c == '=').count();
    let dots = run.len() - dashes - equals;
    (equals >= 2 && dashes == 0 && dots == 0)
        || (equals == 0 && dots == 0 && dashes >= 2)
        || (equals == 0 && dots >= 1 && dashes >= 1 && run.starts_with('-'))
}

/// A link, with its text either inside the operator or between bars after it, and the text that
/// follows.
fn parse_edge(s: &str) -> Option<(Link, &str)> {
    if let Some(rest) = s.strip_prefix("~~~") {
        let link = Link {
            start: Head::None,
            end: Head::None,
            weight: Weight::Normal,
            label: None,
            visible: false,
        };
        return Some((link, rest.trim_start_matches('~')));
    }
    let mut chars = s.chars();
    let (first, second) = (chars.next(), chars.next());
    let start = head_of(first, true).filter(|_| matches!(second, Some('-' | '=' | '.')));
    let body = if start.is_some() { &s[1..] } else { s };
    let run = line_run(body);
    let (line, after) = body.split_at(run);
    if !is_line(line) {
        return None;
    }
    let mut tail = after.chars();
    let end = head_of(tail.next(), false)
        .filter(|h| *h == Head::Arrow || tail.next().is_none_or(|c| c.is_whitespace() || c == '|'));
    let (mut link, rest) = if end.is_some() || run >= 3 {
        let link = Link {
            start: start.unwrap_or(Head::None),
            end: end.unwrap_or(Head::None),
            weight: weight_of(line),
            label: None,
            visible: true,
        };
        (link, if end.is_some() { &after[1..] } else { after })
    } else if after.starts_with(' ') {
        // `-- text -->`: an opening of 2 characters, a space, the text, and a closing line.
        let (at, used, end) = closing(after, line)?;
        let link = Link {
            start: start.unwrap_or(Head::None),
            end,
            weight: weight_of(line),
            label: single_line(&after[..at]),
            visible: true,
        };
        (link, &after[at + used..])
    } else {
        return None;
    };
    let rest = rest.trim_start();
    match rest.strip_prefix('|') {
        Some(inner) => {
            let end = inner.find('|')?;
            link.label = single_line(&inner[..end]);
            Some((link, &inner[end + 1..]))
        }
        None => Some((link, rest)),
    }
}

/// Where the closing line of a link with text starts in `s`, its length and its head.
fn closing(s: &str, open: &str) -> Option<(usize, usize, Head)> {
    let lead = match open {
        "==" => '=',
        "-." => '.',
        _ => '-',
    };
    for (at, c) in s.char_indices() {
        if c != lead || !s[..at].ends_with(' ') {
            continue;
        }
        let rest = &s[at..];
        let run = line_run(rest);
        let line = &rest[..run];
        let ok = match lead {
            '=' => run >= 2,
            '.' => line.trim_start_matches('.').starts_with('-') && run >= 2,
            _ => run >= 2 && !line.contains(['.', '=']),
        };
        if !ok {
            continue;
        }
        let tail = rest[run..].chars().next();
        let head = head_of(tail, false);
        let used = run + usize::from(head.is_some());
        return Some((at, used, head.unwrap_or(Head::None)));
    }
    None
}

/// Edge text, without quotes, a `<br>` written as `\n`. `None` when empty.
fn single_line(text: &str) -> Option<String> {
    let text = text.trim().trim_matches('"');
    let lines = label_lines(text);
    (!lines.is_empty()).then(|| lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_chain() {
        let flow = parse("graph LR\n  A[Start] --> B --- C;\n  B --> A\n").unwrap();
        assert_eq!(flow.direction, Direction::LeftRight);
        assert_eq!(flow.nodes.len(), 3);
        assert_eq!(flow.nodes[0].lines, ["Start"]);
        assert_eq!(flow.nodes[1].lines, ["B"]);
        assert_eq!(flow.edges.len(), 3);
        assert!(flow.edges[0].to_end == Head::Arrow && flow.edges[1].to_end == Head::None);
    }

    #[test]
    fn parses_edge_styles_and_text() {
        let src = "graph TD\n A -.-> B\n B ==> C\n C -->|yes| D\n D -- no --- E\n E == big ==> F\n";
        let flow = parse(src).unwrap();
        let edges: Vec<_> = flow
            .edges
            .iter()
            .map(|e| (e.to_end == Head::Arrow, e.weight, e.label.as_deref()))
            .collect();
        assert_eq!(
            edges,
            [
                (true, Weight::Dotted, None),
                (true, Weight::Thick, None),
                (true, Weight::Normal, Some("yes")),
                (false, Weight::Normal, Some("no")),
                (true, Weight::Thick, Some("big")),
            ]
        );
    }

    #[test]
    fn rejects_other_diagrams_and_unsupported_syntax() {
        assert!(parse("sequenceDiagram\n A->>B: x").is_none());
        assert!(parse("graph TD\n A --> B\n subgraph s\n C\n").is_none());
        assert!(parse("graph TD\n subgraph s\n C\n end\n A --> s").is_none());
        assert!(parse("graph TD\n A --> B\n end").is_none());
        assert!(parse("graph TD\n A[x --> B").is_none());
        assert!(parse("graph XX\n A --> B").is_none());
    }

    #[test]
    fn parses_shapes_and_labels() {
        let flow = parse("graph TD\n A([Go]) --> B{\"Is it?\"} --> C[a<br/>b]\n").unwrap();
        assert_eq!(flow.nodes[0].shape, Shape::Stadium);
        assert_eq!(flow.nodes[1].shape, Shape::Diamond);
        assert_eq!(flow.nodes[1].lines, ["Is it?"]);
        assert_eq!(flow.nodes[2].lines, ["a", "b"]);
    }

    #[test]
    fn parses_nested_subgraphs() {
        let src = "graph LR\n A --> B\n subgraph outer [Outer box]\n  B --> C\n  \
                   subgraph inner\n   C\n   D\n  end\n end\n D --> E\n";
        let flow = parse(src).unwrap();
        assert_eq!(flow.groups.len(), 2);
        assert_eq!(flow.groups[0].title, ["Outer box"]);
        assert_eq!(flow.groups[0].parent, None);
        assert_eq!(flow.groups[1].parent, Some(0));
        let name = |i: usize| flow.nodes[i].lines[0].as_str();
        let members = |g: usize| {
            flow.groups[g]
                .nodes
                .iter()
                .map(|&i| name(i))
                .collect::<Vec<_>>()
        };
        assert_eq!(members(1), ["C", "D"]);
        assert_eq!(members(0), ["B"]);
        assert_eq!(flow.edges.len(), 3);
    }

    #[test]
    fn parses_heads_ampersands_and_long_links() {
        let flow =
            parse("graph TD\n A & B ---> C\n C <--> D\n D --o E\n E -- a b ---x F\n").unwrap();
        let ends: Vec<_> = flow
            .edges
            .iter()
            .map(|e| (e.from, e.to, e.from_end, e.to_end))
            .collect();
        assert_eq!(
            ends,
            [
                (0, 2, Head::None, Head::Arrow),
                (1, 2, Head::None, Head::Arrow),
                (2, 3, Head::Arrow, Head::Arrow),
                (3, 4, Head::None, Head::Circle),
                (4, 5, Head::None, Head::Cross),
            ]
        );
        assert_eq!(flow.edges[4].label.as_deref(), Some("a b"));
    }

    #[test]
    fn edge_text_keeps_its_line_breaks() {
        let flow = parse("graph TD\n A -->|one<br/>two| B\n B -. \"q\" .-> C\n").unwrap();
        assert_eq!(flow.edges[0].label.as_deref(), Some("one\ntwo"));
        assert_eq!(flow.edges[0].text().as_deref(), Some("one two"));
        assert_eq!(flow.edges[1].label.as_deref(), Some("q"));
        assert_eq!(flow.edges[1].weight, Weight::Dotted);
    }
}

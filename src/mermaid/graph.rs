//! Graph shared by the diagrams drawn with the flowchart layout.

use std::collections::HashMap;

use super::canvas::{Dir, Weight};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    TopDown,
    BottomUp,
    LeftRight,
    RightLeft,
}

impl Direction {
    pub fn parse(token: &str) -> Option<Direction> {
        match token {
            "TD" | "TB" => Some(Direction::TopDown),
            "BT" => Some(Direction::BottomUp),
            "LR" => Some(Direction::LeftRight),
            "RL" => Some(Direction::RightLeft),
            _ => None,
        }
    }

    pub fn horizontal(self) -> bool {
        matches!(self, Direction::LeftRight | Direction::RightLeft)
    }

    /// Direction in which edges between layers leave a node.
    pub fn flow(self) -> Dir {
        match self {
            Direction::TopDown => Dir::S,
            Direction::BottomUp => Dir::N,
            Direction::LeftRight => Dir::E,
            Direction::RightLeft => Dir::W,
        }
    }

    /// Side used by edges that go back to an earlier layer.
    pub fn side(self) -> Dir {
        if self.horizontal() { Dir::S } else { Dir::E }
    }
}

/// What a node stands for, from its shape: it gives the node its colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Step,
    End,
    Decision,
    Store,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Rect,
    Round,
    Diamond,
    /// Rectangle with half circles at both ends: `([text])`.
    Stadium,
    /// `((text))`.
    Circle,
    /// `(((text)))`.
    DoubleCircle,
    /// Rectangle with a double line on each side: `[[text]]`.
    Subroutine,
    /// Database cylinder: `[(text)]`.
    Cylinder,
    /// `{{text}}`.
    Hexagon,
    /// Flag pointed on the left: `>text]`.
    Asymmetric,
    /// Parallelogram leaning right `[/text/]`, or left `[\text\]`.
    Lean(bool),
    /// Trapezoid wider at the bottom `[/text\]`, or at the top `[\text/]`.
    Trapezoid(bool),
    /// Initial pseudo-state of a state diagram.
    Start,
    /// Final pseudo-state of a state diagram.
    End,
}

/// End of an edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Head {
    None,
    Arrow,
    /// Hollow triangle of a class inheritance.
    Inherit,
    /// Filled diamond of a composition.
    Composition,
    /// Hollow diamond of an aggregation.
    Aggregation,
    /// Circle of a flowchart link `--o`.
    Circle,
    /// Cross of a flowchart link `--x`.
    Cross,
    /// Cardinality of an entity relation: exactly one, `||`.
    One,
    /// Zero or one, `|o`.
    ZeroOne,
    /// One or more, `}|`.
    OneMany,
    /// Zero or more, `}o`.
    ZeroMany,
}

impl Head {
    /// The cardinality that an entity relation end stands for, as the grid drawing writes it.
    fn count(self) -> Option<&'static str> {
        match self {
            Head::One => Some("1"),
            Head::ZeroOne => Some("0..1"),
            Head::OneMany => Some("1..*"),
            Head::ZeroMany => Some("0..*"),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Node {
    pub lines: Vec<String>,
    /// Indices in `lines` before which a divider row is drawn.
    pub dividers: Vec<usize>,
    pub shape: Shape,
}

/// An edge laid out from `from` to `to`. The ends may carry a head each.
#[derive(Debug, PartialEq, Eq)]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    pub from_end: Head,
    pub to_end: Head,
    pub weight: Weight,
    /// Text of the edge. A line break of the source is a `\n`.
    pub label: Option<String>,
}

impl Shape {
    /// An end for a stadium, a circle and a start or end state, a decision for a diamond and a
    /// hexagon, a store for a cylinder, else a step.
    pub fn role(self) -> Role {
        match self {
            Shape::Stadium | Shape::Circle | Shape::DoubleCircle | Shape::Start | Shape::End => {
                Role::End
            }
            Shape::Diamond | Shape::Hexagon => Role::Decision,
            Shape::Cylinder => Role::Store,
            _ => Role::Step,
        }
    }
}

impl Edge {
    /// The index of the first edge of `edges` drawn like this one: same line, same ends.
    pub fn look(&self, edges: &[Edge]) -> usize {
        edges
            .iter()
            .position(|e| {
                (e.weight, e.from_end, e.to_end) == (self.weight, self.from_end, self.to_end)
            })
            .unwrap_or(0)
    }

    /// The label on one line, with the cardinalities of an entity relation after it.
    pub fn text(&self) -> Option<String> {
        let label = self.label.as_ref().map(|l| l.replace('\n', " "));
        match (self.from_end.count(), self.to_end.count()) {
            (Some(a), Some(b)) => {
                let counts = format!("({a} to {b})");
                Some(label.map_or(counts.clone(), |l| format!("{l} {counts}")))
            }
            _ => label,
        }
    }
}

/// A subgraph: a titled box around its nodes and the subgraphs inside it.
#[derive(Debug, PartialEq, Eq)]
pub struct Group {
    pub title: Vec<String>,
    /// The subgraph that holds this one.
    pub parent: Option<usize>,
    /// The nodes that belong to this subgraph and to none inside it.
    pub nodes: Vec<usize>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Flow {
    pub direction: Direction,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    /// Subgraphs, each after the one that holds it. The grid drawing does not draw them.
    pub groups: Vec<Group>,
}

/// Builds a `Flow`, creating a node on its first mention.
pub struct Builder {
    pub flow: Flow,
    ids: HashMap<String, usize>,
    /// Nodes named since the caller last emptied it.
    pub named: Vec<usize>,
}

impl Builder {
    pub fn new(direction: Direction) -> Builder {
        Builder {
            flow: Flow {
                direction,
                nodes: Vec::new(),
                edges: Vec::new(),
                groups: Vec::new(),
            },
            ids: HashMap::new(),
            named: Vec::new(),
        }
    }

    /// Whether a node is named `id`.
    pub fn has(&self, id: &str) -> bool {
        self.ids.contains_key(id)
    }

    /// Index of the node named `id`, created as a rectangle labelled `id` when new.
    pub fn node(&mut self, id: &str) -> usize {
        let index = *self.ids.entry(id.to_string()).or_insert_with(|| {
            self.flow.nodes.push(Node {
                lines: vec![id.to_string()],
                dividers: Vec::new(),
                shape: Shape::Rect,
            });
            self.flow.nodes.len() - 1
        });
        self.named.push(index);
        index
    }

    /// A node that is not looked up by name.
    pub fn anonymous(&mut self, shape: Shape) -> usize {
        self.flow.nodes.push(Node {
            lines: Vec::new(),
            dividers: Vec::new(),
            shape,
        });
        self.flow.nodes.len() - 1
    }
}

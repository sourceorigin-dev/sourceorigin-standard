//! YAML documents with the line of each node, which `yaml_rust2::Yaml`
//! drops, built from the parser's events.

use std::collections::BTreeMap;

use yaml_rust2::parser::{MarkedEventReceiver, Parser};
use yaml_rust2::scanner::{Marker, TScalarStyle};
use yaml_rust2::{Event, ScanError};

/// A node of a YAML document.
#[derive(Clone, Debug)]
pub(super) struct Node {
    /// The line it starts on, from 1.
    pub(super) line: usize,
    /// The character of that line it starts at, from 0: for a quoted
    /// scalar, its opening quote.
    pub(super) column: usize,
    pub(super) value: Value,
}

/// How a scalar is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Style {
    Plain,
    /// In `'…'` or `"…"`.
    Quoted,
    /// A `|` block, its lines kept.
    Literal,
    /// A `>` block, its lines folded into one.
    Folded,
}

/// A node's value.
#[derive(Clone, Debug)]
pub(super) enum Value {
    /// A scalar and its style. A `|` or `>` block's node starts on its
    /// text's first line rather than on the indicator's.
    Scalar(String, Style),
    Sequence(Vec<Node>),
    /// Each key with its value.
    Mapping(Vec<(Node, Node)>),
}

impl Node {
    /// The document `text` holds, or `None` for an empty one.
    pub(super) fn parse(text: &str) -> Result<Option<Self>, ScanError> {
        let mut builder = Builder::default();
        Parser::new_from_str(text).load(&mut builder, false)?;
        Ok(builder.document)
    }

    /// The value of `key`, when it is a mapping holding it.
    pub(super) fn get(&self, key: &str) -> Option<&Self> {
        self.entries()
            .iter()
            .find(|(name, _)| name.as_str() == Some(key))
            .map(|(_, value)| value)
    }

    /// Its text, when it is a scalar.
    pub(super) fn as_str(&self) -> Option<&str> {
        match &self.value {
            Value::Scalar(text, _) => Some(text),
            _ => None,
        }
    }

    /// Its style, when it is a scalar.
    pub(super) fn style(&self) -> Option<Style> {
        match &self.value {
            Value::Scalar(_, style) => Some(*style),
            Value::Sequence(_) | Value::Mapping(_) => None,
        }
    }

    /// Its items, when it is a sequence.
    pub(super) fn items(&self) -> &[Self] {
        match &self.value {
            Value::Sequence(items) => items,
            _ => &[],
        }
    }

    /// Its keys and values, when it is a mapping.
    pub(super) fn entries(&self) -> &[(Self, Self)] {
        match &self.value {
            Value::Mapping(entries) => entries,
            _ => &[],
        }
    }

    /// Each line of a scalar, with its line in the file. A quoted scalar
    /// spanning lines counts as on its first.
    pub(super) fn lines(&self) -> Vec<(usize, &str)> {
        match &self.value {
            Value::Scalar(text, Style::Literal | Style::Folded) => text
                .lines()
                .enumerate()
                .map(|(index, line)| (self.line + index, line))
                .collect(),
            Value::Scalar(text, Style::Plain | Style::Quoted) => {
                text.lines().map(|line| (self.line, line)).collect()
            }
            _ => Vec::new(),
        }
    }
}

/// Builds the first document from the parser's events, resolving aliases.
#[derive(Default)]
struct Builder {
    /// The collections being built, innermost last.
    open: Vec<Open>,
    anchors: BTreeMap<usize, Node>,
    document: Option<Node>,
}

/// A collection being built.
struct Open {
    line: usize,
    column: usize,
    anchor: usize,
    items: Vec<Node>,
    /// Whether it is a mapping, whose items alternate keys and values.
    mapping: bool,
}

impl Builder {
    /// Adds a finished node to the collection being built, or makes it the
    /// document.
    fn finish(&mut self, node: Node, anchor: usize) {
        if anchor > 0 {
            self.anchors.insert(anchor, node.clone());
        }
        match self.open.last_mut() {
            Some(open) => open.items.push(node),
            None => {
                self.document.get_or_insert(node);
            }
        }
    }
}

impl MarkedEventReceiver for Builder {
    fn on_event(&mut self, event: Event, marker: Marker) {
        let line = marker.line();
        match event {
            Event::Scalar(text, style, anchor, _) => {
                let style = match style {
                    TScalarStyle::Literal => Style::Literal,
                    TScalarStyle::Folded => Style::Folded,
                    TScalarStyle::SingleQuoted | TScalarStyle::DoubleQuoted => Style::Quoted,
                    TScalarStyle::Plain => Style::Plain,
                };
                self.finish(
                    Node {
                        line,
                        column: marker.col(),
                        value: Value::Scalar(text, style),
                    },
                    anchor,
                );
            }
            Event::SequenceStart(anchor, _) | Event::MappingStart(anchor, _) => {
                self.open.push(Open {
                    line,
                    column: marker.col(),
                    anchor,
                    items: Vec::new(),
                    mapping: matches!(event, Event::MappingStart(..)),
                });
            }
            Event::SequenceEnd | Event::MappingEnd => {
                if let Some(open) = self.open.pop() {
                    let value = if open.mapping {
                        let mut items = open.items.into_iter();
                        Value::Mapping(
                            std::iter::from_fn(|| Some((items.next()?, items.next()?))).collect(),
                        )
                    } else {
                        Value::Sequence(open.items)
                    };
                    self.finish(
                        Node {
                            line: open.line,
                            column: open.column,
                            value,
                        },
                        open.anchor,
                    );
                }
            }
            Event::Alias(anchor) => {
                if let Some(node) = self.anchors.get(&anchor).cloned() {
                    self.finish(node, 0);
                }
            }
            _ => {}
        }
    }
}

//! Recursive descent only follows explicit grouping, capped at depth 64.
//! Concatenation and choice remain graph edges; no string expansion occurs.

use super::graph::{Builder, Graph, Quota};
use crate::{SearchError, capacity::Budget};

#[derive(Clone, Copy)]
struct Fragment {
    start: usize,
    end: usize,
}

struct Parser<'a, 'b> {
    input: &'a [u16],
    at: usize,
    depth: usize,
    graph: Builder,
    budget: &'b mut Budget,
    quota: &'b mut Quota,
}

pub(super) fn parse(
    input: &[u16],
    budget: &mut Budget,
    quota: &mut Quota,
) -> Result<Graph, SearchError> {
    if input.len() > 65535 {
        return Err(SearchError::ResourceLimit("intent expression unit limit"));
    }
    let mut parser = Parser {
        input,
        at: 0,
        depth: 0,
        graph: Builder::default(),
        budget,
        quota,
    };
    let fragment = parser.expression()?;
    if parser.at != input.len() {
        return Err(SearchError::InvalidInput(
            "unmatched intent closing parenthesis",
        ));
    }
    parser
        .graph
        .finish(fragment.start, fragment.end, parser.budget)
}

impl Parser<'_, '_> {
    fn peek(&self) -> Option<u16> {
        self.input.get(self.at).copied()
    }

    fn expression(&mut self) -> Result<Fragment, SearchError> {
        let mut left = self.sequence()?;
        while self.peek() == Some(b'|' as u16) {
            self.at += 1;
            let right = self.sequence()?;
            let start = self.graph.node(self.quota)?;
            let end = self.graph.node(self.quota)?;
            self.graph
                .epsilon(start, left.start, self.budget, self.quota)?;
            self.graph
                .epsilon(start, right.start, self.budget, self.quota)?;
            self.graph.epsilon(left.end, end, self.budget, self.quota)?;
            self.graph
                .epsilon(right.end, end, self.budget, self.quota)?;
            left = Fragment { start, end };
        }
        Ok(left)
    }

    fn sequence(&mut self) -> Result<Fragment, SearchError> {
        let start = self.graph.node(self.quota)?;
        let mut end = start;
        while !matches!(self.peek(), None | Some(0x29 | 0x7c)) {
            let atom = self.atom()?;
            self.graph
                .epsilon(end, atom.start, self.budget, self.quota)?;
            end = atom.end;
        }
        Ok(Fragment { start, end })
    }

    fn atom(&mut self) -> Result<Fragment, SearchError> {
        let fragment = match self.peek() {
            Some(0x28) => {
                if self.depth >= 64 {
                    return Err(SearchError::ResourceLimit("intent group depth"));
                }
                self.depth += 1;
                self.at += 1;
                let inner = self.expression()?;
                if self.peek() != Some(0x29) {
                    return Err(SearchError::InvalidInput("unclosed intent group"));
                }
                self.at += 1;
                self.depth -= 1;
                inner
            }
            Some(0x3f | 0x2a | 0x2b) => {
                return Err(SearchError::InvalidInput(
                    "unsupported or misplaced intent quantifier",
                ));
            }
            Some(_) => {
                if self.peek() == Some(0x5c) {
                    self.at += 1;
                    if self.peek().is_none() {
                        return Err(SearchError::InvalidInput("trailing intent escape"));
                    }
                }
                let start = self.graph.node(self.quota)?;
                let mut end = start;
                let first = self.input[self.at];
                let width = if (0xd800..=0xdbff).contains(&first)
                    && self
                        .input
                        .get(self.at + 1)
                        .is_some_and(|unit| (0xdc00..=0xdfff).contains(unit))
                {
                    2
                } else {
                    1
                };
                for _ in 0..width {
                    let unit = self.input[self.at];
                    self.at += 1;
                    let to = self.graph.node(self.quota)?;
                    self.graph.edge(
                        super::graph::Edge {
                            from: end,
                            to,
                            symbol: u32::from(unit),
                            raw: [unit, 0],
                            raw_len: 1,
                        },
                        self.budget,
                        self.quota,
                    )?;
                    end = to;
                }
                Fragment { start, end }
            }
            None => return Err(SearchError::InvalidInput("missing intent atom")),
        };
        if self.peek() == Some(0x3f) {
            self.at += 1;
            self.graph
                .epsilon(fragment.start, fragment.end, self.budget, self.quota)?;
            if self.peek() == Some(0x3f) {
                return Err(SearchError::InvalidInput(
                    "repeated intent optional quantifier",
                ));
            }
        }
        Ok(fragment)
    }
}

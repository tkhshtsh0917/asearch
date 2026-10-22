//! Shortest paths on (DAG node, pattern position). Every graph move advances
//! topologically; optional/choice paths merge by minimum cost, never expansion.

use super::graph::{EPSILON, Graph};
use crate::{
    SearchError,
    capacity::{Budget, add, mul},
    search::classes::Class,
};

#[derive(Clone, Copy, Debug, Default)]
struct Previous {
    node: usize,
    column: usize,
    edge: Option<usize>,
}

pub(super) fn level(
    graph: &Graph,
    classes: &[Class],
    leading: bool,
    cap: u8,
    cells: &mut [u8],
) -> u8 {
    evaluate(graph, classes, leading, cap, cells, None)
}

fn evaluate(
    graph: &Graph,
    classes: &[Class],
    leading: bool,
    cap: u8,
    cells: &mut [u8],
    mut previous: Option<&mut [Previous]>,
) -> u8 {
    let width = classes.len() + 1;
    let count = graph.nodes() * width;
    cells[..count].fill(cap);
    cells[graph.start * width] = 0;
    for node in 0..graph.nodes() {
        for j in 1..width {
            let cost = cells[node * width + j - 1].saturating_add(1).min(cap);
            improve(
                cells,
                &mut previous,
                node * width + j,
                cost,
                Previous {
                    node,
                    column: j - 1,
                    edge: None,
                },
            );
        }
        for edge_id in graph.outgoing(node) {
            let edge = graph.edges[edge_id];
            for j in 0..width {
                let old = cells[node * width + j];
                let predecessor = Previous {
                    node,
                    column: j,
                    edge: Some(edge_id),
                };
                if edge.symbol == EPSILON {
                    improve(cells, &mut previous, edge.to * width + j, old, predecessor);
                    continue;
                }
                let looping = if j == 0 {
                    leading
                } else {
                    classes[j - 1].looping
                };
                improve(
                    cells,
                    &mut previous,
                    edge.to * width + j,
                    old.saturating_add(u8::from(!looping)).min(cap),
                    predecessor,
                );
                if let Some(class) = classes.get(j) {
                    let cost = u8::from(!class.keys.contains(&edge.symbol));
                    improve(
                        cells,
                        &mut previous,
                        edge.to * width + j + 1,
                        old.saturating_add(cost).min(cap),
                        predecessor,
                    );
                }
            }
        }
    }
    cells[graph.end * width + classes.len()]
}

fn improve(
    cells: &mut [u8],
    previous: &mut Option<&mut [Previous]>,
    at: usize,
    cost: u8,
    from: Previous,
) {
    if cost < cells[at] {
        cells[at] = cost;
        if let Some(previous) = previous {
            previous[at] = from;
        }
    }
}

pub(super) fn expansion(
    graph: &Graph,
    classes: &[Class],
    leading: bool,
    k: u8,
    max_units: usize,
    available: usize,
) -> Result<Option<(Vec<u16>, u16)>, SearchError> {
    let mut budget = Budget::new(0, available)?;
    let width = add(classes.len(), 1)?;
    let count = mul(graph.nodes(), width)?;
    let mut cells = budget.reserve(count)?;
    cells.resize(count, 0);
    let mut previous = budget.reserve(count)?;
    previous.resize(count, Previous::default());
    let mut text = budget.reserve(max_units)?;
    let level = evaluate(
        graph,
        classes,
        leading,
        k + 1,
        &mut cells,
        Some(&mut previous),
    );
    if level > k {
        return Ok(None);
    }
    let mut node = graph.end;
    let mut column = classes.len();
    while node != graph.start || column != 0 {
        let predecessor = previous[node * width + column];
        if let Some(edge) = predecessor.edge {
            let edge = graph.edges[edge];
            if text.len() + usize::from(edge.raw_len) > max_units {
                return Err(SearchError::ResourceLimit("intent expansion unit limit"));
            }
            text.extend(edge.raw[..usize::from(edge.raw_len)].iter().rev());
        }
        node = predecessor.node;
        column = predecessor.column;
    }
    text.reverse();
    Ok(Some((text, u16::from(level))))
}

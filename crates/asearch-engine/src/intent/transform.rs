//! Finite carry states make normalization/surrogate composition cross group
//! boundaries without enumerating complete expression expansions.
use super::graph::{Builder, EPSILON, Edge, Graph, Quota, push};
use crate::{Normalizer, SearchError, capacity::Budget};
const NONE: u32 = u32::MAX;
#[derive(Clone, Copy)]
struct State {
    pending: u32,
    node: usize,
}
#[derive(Clone, Copy)]
struct Emission {
    symbol: u32,
    raw: [u16; 2],
    len: u8,
}
impl Emission {
    const EMPTY: Self = Self {
        symbol: EPSILON,
        raw: [0; 2],
        len: 0,
    };
    fn unit(symbol: u32, raw: [u16; 2], len: u8) -> Self {
        Self { symbol, raw, len }
    }
}
struct Transition {
    pending: u32,
    emit: [Emission; 2],
    count: usize,
}
impl Transition {
    fn zero(pending: u32) -> Self {
        Self {
            pending,
            emit: [Emission::EMPTY; 2],
            count: 0,
        }
    }
    fn one(pending: u32, emission: Emission) -> Self {
        Self {
            pending,
            emit: [emission, Emission::EMPTY],
            count: 1,
        }
    }
}
#[derive(Clone, Copy)]
pub(super) enum Kind {
    Normalize(Normalizer),
    Symbols,
}

pub(super) fn compile(
    graph: Graph,
    kind: Kind,
    budget: &mut Budget,
    quota: &mut Quota,
) -> Result<Graph, SearchError> {
    let mut variants = budget.reserve(graph.nodes())?;
    variants.resize_with(graph.nodes(), Vec::<State>::new);
    let mut output = Builder::default();
    let start = variant(&mut variants[graph.start], NONE, &mut output, budget, quota)?;
    let mut buffer = budget.reserve(2)?;
    for node in 0..graph.nodes() {
        for position in 0..variants[node].len() {
            let state = variants[node][position];
            for edge in &graph.edges[graph.outgoing(node)] {
                let transition = step(kind, state.pending, edge, &mut buffer)?;
                let to = variant(
                    &mut variants[edge.to],
                    transition.pending,
                    &mut output,
                    budget,
                    quota,
                )?;
                connect(&mut output, state.node, to, &transition, budget, quota)?;
            }
        }
    }
    let end = output.node(quota)?;
    for state in &variants[graph.end] {
        let transition = flush(kind, state.pending, &mut buffer)?;
        connect(&mut output, state.node, end, &transition, budget, quota)?;
    }
    budget.release(buffer);
    for states in variants.drain(..) {
        budget.release(states);
    }
    budget.release(variants);
    let output = output.finish(start, end, budget)?;
    graph.release(budget);
    Ok(output)
}
fn variant(
    states: &mut Vec<State>,
    pending: u32,
    output: &mut Builder,
    budget: &mut Budget,
    quota: &mut Quota,
) -> Result<usize, SearchError> {
    if let Some(state) = states.iter().find(|state| state.pending == pending) {
        return Ok(state.node);
    }
    if states.len() >= 128 {
        return Err(SearchError::ResourceLimit("intent carry-state variants"));
    }
    let node = output.node(quota)?;
    push(states, State { pending, node }, budget)?;
    Ok(node)
}
fn connect(
    output: &mut Builder,
    from: usize,
    to: usize,
    transition: &Transition,
    budget: &mut Budget,
    quota: &mut Quota,
) -> Result<(), SearchError> {
    if transition.count == 0 {
        return output.epsilon(from, to, budget, quota);
    }
    let mut previous = from;
    for (i, emission) in transition.emit[..transition.count].iter().enumerate() {
        let next = if i + 1 == transition.count {
            to
        } else {
            output.node(quota)?
        };
        output.edge(
            Edge {
                from: previous,
                to: next,
                symbol: emission.symbol,
                raw: emission.raw,
                raw_len: emission.len,
            },
            budget,
            quota,
        )?;
        previous = next;
    }
    Ok(())
}
fn step(
    kind: Kind,
    pending: u32,
    edge: &Edge,
    buffer: &mut Vec<u16>,
) -> Result<Transition, SearchError> {
    if edge.symbol == EPSILON {
        return Ok(Transition::zero(pending));
    }
    match kind {
        Kind::Normalize(normalizer) => {
            let unit = u16::try_from(edge.symbol)
                .map_err(|_| SearchError::InvalidInput("invalid normalization graph symbol"))?;
            if normalizer.descriptor().2 & 4 == 0 {
                normalizer
                    .normalize_into(&[unit], buffer)
                    .map_err(|_| SearchError::ResourceLimit("intent normalization buffer"))?;
                return Ok(Transition::one(
                    NONE,
                    Emission::unit(u32::from(buffer[0]), [unit, 0], 1),
                ));
            }
            if pending == NONE {
                return Ok(Transition::zero(u32::from(unit)));
            }
            let previous = pending as u16;
            normalizer
                .normalize_into(&[previous, unit], buffer)
                .map_err(|_| SearchError::ResourceLimit("intent normalization buffer"))?;
            if buffer.len() == 1 {
                Ok(Transition::one(
                    NONE,
                    Emission::unit(u32::from(buffer[0]), [previous, unit], 2),
                ))
            } else {
                Ok(Transition::one(
                    u32::from(unit),
                    Emission::unit(u32::from(buffer[0]), [previous, 0], 1),
                ))
            }
        }
        Kind::Symbols => {
            let unit = edge.symbol;
            if pending == NONE {
                if (0xd800..=0xdbff).contains(&unit) {
                    Ok(Transition::zero(unit))
                } else {
                    Ok(Transition::one(
                        NONE,
                        Emission::unit(unit, edge.raw, edge.raw_len),
                    ))
                }
            } else if (0xdc00..=0xdfff).contains(&unit) {
                let symbol = 0x10000 + ((pending - 0xd800) << 10) + (unit - 0xdc00);
                Ok(Transition::one(
                    NONE,
                    Emission::unit(symbol, [pending as u16, unit as u16], 2),
                ))
            } else {
                let previous = Emission::unit(pending, [pending as u16, 0], 1);
                if (0xd800..=0xdbff).contains(&unit) {
                    Ok(Transition::one(unit, previous))
                } else {
                    Ok(Transition {
                        pending: NONE,
                        emit: [previous, Emission::unit(unit, edge.raw, edge.raw_len)],
                        count: 2,
                    })
                }
            }
        }
    }
}
fn flush(kind: Kind, pending: u32, buffer: &mut Vec<u16>) -> Result<Transition, SearchError> {
    if pending == NONE {
        return Ok(Transition::zero(NONE));
    }
    let symbol = match kind {
        Kind::Normalize(normalizer) => {
            normalizer
                .normalize_into(&[pending as u16], buffer)
                .map_err(|_| SearchError::ResourceLimit("intent normalization buffer"))?;
            u32::from(buffer[0])
        }
        Kind::Symbols => pending,
    };
    Ok(Transition::one(
        NONE,
        Emission::unit(symbol, [pending as u16, 0], 1),
    ))
}

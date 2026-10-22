//! Stores acyclic expression graphs and accounts for node/edge construction quotas.

use crate::{
    SearchError,
    capacity::{Budget, add},
};
use std::ops::Range;
pub(super) const EPSILON: u32 = u32::MAX;

#[derive(Clone, Copy, Debug)]
pub(super) struct Edge {
    pub from: usize,
    pub to: usize,
    pub symbol: u32,
    pub raw: [u16; 2],
    pub raw_len: u8,
}

#[derive(Debug)]
pub(super) struct Graph {
    pub offsets: Vec<usize>,
    pub edges: Vec<Edge>,
    pub start: usize,
    pub end: usize,
}

impl Graph {
    pub fn nodes(&self) -> usize {
        self.offsets.len() - 1
    }

    pub fn outgoing(&self, node: usize) -> Range<usize> {
        self.offsets[node]..self.offsets[node + 1]
    }

    pub fn length_bounds(&self, budget: &mut Budget) -> Result<(usize, usize), SearchError> {
        let mut minimum = budget.reserve(self.nodes())?;
        minimum.resize(self.nodes(), usize::MAX);
        let mut maximum = budget.reserve(self.nodes())?;
        maximum.resize(self.nodes(), 0usize);
        minimum[self.start] = 0;

        for node in 0..self.nodes() {
            if minimum[node] == usize::MAX {
                continue;
            }
            for edge in &self.edges[self.outgoing(node)] {
                let consumed = usize::from(edge.symbol != EPSILON);
                minimum[edge.to] = minimum[edge.to].min(minimum[node] + consumed);
                maximum[edge.to] = maximum[edge.to].max(maximum[node] + consumed);
            }
        }

        let result = (minimum[self.end], maximum[self.end]);
        budget.release(minimum);
        budget.release(maximum);
        if result.0 == usize::MAX {
            return Err(SearchError::InvalidInput("unreachable intent terminal"));
        }

        Ok(result)
    }

    pub fn release(self, budget: &mut Budget) {
        budget.release(self.offsets);
        budget.release(self.edges);
    }
}

#[derive(Default)]
pub(super) struct Builder {
    pub nodes: usize,
    pub edges: Vec<Edge>,
}
pub(super) struct Quota {
    nodes: usize,
    edges: usize,
    max_nodes: usize,
    max_edges: usize,
}

impl Quota {
    pub fn raw(slots: usize) -> Self {
        Self {
            nodes: 0,
            edges: 0,
            max_nodes: slots.min(262144),
            max_edges: 524288,
        }
    }

    pub fn compiled(slots: usize) -> Self {
        Self {
            nodes: 0,
            edges: 0,
            max_nodes: slots.min(524288),
            max_edges: 1048576,
        }
    }
}

impl Builder {
    pub fn node(&mut self, quota: &mut Quota) -> Result<usize, SearchError> {
        if quota.nodes >= quota.max_nodes {
            return Err(SearchError::ResourceLimit("intent node budget"));
        }
        quota.nodes += 1;
        let node = self.nodes;
        self.nodes += 1;
        Ok(node)
    }

    pub fn edge(
        &mut self,
        edge: Edge,
        budget: &mut Budget,
        quota: &mut Quota,
    ) -> Result<(), SearchError> {
        if edge.from >= self.nodes || edge.to >= self.nodes || edge.from == edge.to {
            return Err(SearchError::InvalidInput("invalid intent graph edge"));
        }
        if quota.edges >= quota.max_edges {
            return Err(SearchError::ResourceLimit("intent edge budget"));
        }
        quota.edges += 1;
        push(&mut self.edges, edge, budget)
    }

    pub fn epsilon(
        &mut self,
        from: usize,
        to: usize,
        budget: &mut Budget,
        quota: &mut Quota,
    ) -> Result<(), SearchError> {
        if from != to {
            self.edge(
                Edge {
                    from,
                    to,
                    symbol: EPSILON,
                    raw: [0; 2],
                    raw_len: 0,
                },
                budget,
                quota,
            )?;
        }
        Ok(())
    }

    pub fn finish(
        mut self,
        start: usize,
        end: usize,
        budget: &mut Budget,
    ) -> Result<Graph, SearchError> {
        self.edges
            .sort_unstable_by_key(|e| (e.from, e.to, e.symbol, e.raw));
        let mut offsets = budget.reserve(add(self.nodes, 1)?)?;
        offsets.resize(self.nodes + 1, 0);
        let mut indegree = budget.reserve(self.nodes)?;
        indegree.resize(self.nodes, 0usize);
        for edge in &self.edges {
            offsets[edge.from + 1] += 1;
            indegree[edge.to] += 1;
        }
        for i in 1..offsets.len() {
            offsets[i] += offsets[i - 1];
        }
        let mut order = budget.reserve(self.nodes)?;
        for (node, &degree) in indegree.iter().enumerate() {
            if degree == 0 {
                order.push(node);
            }
        }
        let mut cursor = 0;
        while cursor < order.len() {
            let node = order[cursor];
            cursor += 1;
            for edge in &self.edges[offsets[node]..offsets[node + 1]] {
                indegree[edge.to] -= 1;
                if indegree[edge.to] == 0 {
                    order.push(edge.to);
                }
            }
        }
        if order.len() != self.nodes {
            return Err(SearchError::InvalidInput("cyclic intent graph"));
        }
        // Reuse indegree as the old-to-topological position map.
        for (position, &node) in order.iter().enumerate() {
            indegree[node] = position;
        }
        for edge in &mut self.edges {
            edge.from = indegree[edge.from];
            edge.to = indegree[edge.to];
        }
        let start = indegree[start];
        let end = indegree[end];
        budget.release(indegree);
        budget.release(order);
        self.edges
            .sort_unstable_by_key(|e| (e.from, e.to, e.symbol, e.raw));
        offsets.fill(0);
        for edge in &self.edges {
            offsets[edge.from + 1] += 1;
        }
        for i in 1..offsets.len() {
            offsets[i] += offsets[i - 1];
        }
        Ok(Graph {
            offsets,
            edges: self.edges,
            start,
            end,
        })
    }
}

/// Grow through a separately budgeted allocation, including the temporary
/// overlap of old and new capacities. No untracked Vec doubling is allowed.
pub(super) fn push<T>(
    values: &mut Vec<T>,
    value: T,
    budget: &mut Budget,
) -> Result<(), SearchError> {
    if values.len() == values.capacity() {
        let count = values.capacity().saturating_mul(2).max(1);
        let mut next = budget.reserve(count)?;
        next.append(values);
        let old = std::mem::replace(values, next);
        budget.release(old);
    }
    values.push(value);
    Ok(())
}

//! Reconstructs a deterministic alignment path from bounded edit costs.

use super::{Class, Symbol};

// Unset start means no text has been consumed by a paid/literal transition.
// Free wildcard and Include transitions never extend the highlighted span.
#[derive(Clone, Copy, Debug)]
pub(super) struct Cell {
    cost: u32,
    start: usize,
    step: u8,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            cost: 0,
            start: usize::MAX,
            step: 0,
        }
    }
}

impl Cell {
    fn candidate(mut self, cost: u32, step: u8, consumed: Option<usize>) -> Self {
        self.cost += cost;
        self.step = step;
        if let Some(j) = consumed
            && self.start == usize::MAX
        {
            self.start = j - 1;
        }
        self
    }

    fn rank(self) -> (u32, std::cmp::Reverse<usize>, u8) {
        (self.cost, std::cmp::Reverse(self.start), self.step)
    }
}

/// Ties: leftmost end, rightmost start, then exact diagonal, substitution,
/// deletion, insertion and free loop. All predecessor moves decrease i or j.
pub(super) fn align(
    classes: &[Class],
    symbols: &[Symbol],
    leading: bool,
    cells: &mut [Cell],
    selected: &mut Vec<Symbol>,
) -> u16 {
    let width = symbols.len() + 1;
    for j in 1..width {
        cells[j] = cells[j - 1].candidate(
            u32::from(!leading),
            if leading { 5 } else { 4 },
            (!leading).then_some(j),
        );
    }
    for i in 1..=classes.len() {
        cells[i * width] = cells[(i - 1) * width].candidate(1, 3, None);
        for j in 1..width {
            let class = classes[i - 1];
            let cost = u32::from(!class.keys.contains(&symbols[j - 1].value));
            let diagonal = cells[(i - 1) * width + j - 1].candidate(
                cost,
                if cost == 0 { 1 } else { 2 },
                Some(j),
            );
            let deletion = cells[(i - 1) * width + j].candidate(1, 3, None);
            let insertion = cells[i * width + j - 1].candidate(1, 4, Some(j));
            let mut best = [diagonal, deletion, insertion]
                .into_iter()
                .min_by_key(|cell| cell.rank())
                .expect("three candidates");
            if class.looping && i < classes.len() {
                let free = cells[i * width + j - 1].candidate(0, 5, None);
                if free.rank() < best.rank() {
                    best = free;
                }
            }
            cells[i * width + j] = best;
        }
    }
    selected.clear();
    let mut i = classes.len();
    let terminal_loop = classes.last().map_or(leading, |class| class.looping);
    // Choose the endpoint globally. Prefix cells retain the rightmost start;
    // preferring an early prefix endpoint here would lose later short matches
    // when another literal follows an internal wildcard.
    let mut j = if terminal_loop {
        (0..width)
            .min_by_key(|&j| (cells[i * width + j].cost, j, cells[i * width + j].rank()))
            .expect("at least one endpoint")
    } else {
        symbols.len()
    };
    let level = cells[i * width + j].cost;
    while i > 0 || j > 0 {
        match cells[i * width + j].step {
            1 | 2 => {
                selected.push(symbols[j - 1]);
                i -= 1;
                j -= 1;
            }
            3 => {
                i -= 1;
            }
            4 => {
                selected.push(symbols[j - 1]);
                j -= 1;
            }
            5 => {
                j -= 1;
            }
            _ => break,
        }
    }
    u16::try_from(level).unwrap_or(u16::MAX)
}

//! Finds the earliest start at minimum edit cost using one budgeted DP row.

use super::AsearchIndex;
use crate::{Profile, transposed::workspace::Class};

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Cell {
    cost: usize,
    start: usize,
}

impl Cell {
    fn paid(self, cost: usize, consumed: Option<usize>) -> Self {
        Self {
            cost: self.cost + cost,
            start: if self.start == usize::MAX {
                consumed.unwrap_or(usize::MAX)
            } else {
                self.start
            },
        }
    }

    fn rank(self) -> (usize, usize) {
        (self.cost, self.start)
    }
}

/// Literal/paid transitions consume the span; free wildcard loops do not.
/// Coordinates refer to normalized UTF-16, independently of display highlighting.
pub(super) fn start(
    index: &AsearchIndex,
    text_id: usize,
    classes: &[Class],
    leading: bool,
    profile: Profile,
    row: &mut [Cell],
) -> usize {
    let row = &mut row[..=classes.len()];
    for (i, cell) in row.iter_mut().enumerate() {
        *cell = Cell {
            cost: i,
            start: usize::MAX,
        };
    }
    let text = index.text(text_id).expect("validated text ID");
    let mut offset = 0;
    let mut consume = |value: u32, width: usize| {
        let id = index.engine.symbol_id(value);
        let mut diagonal = row[0];
        row[0] = if leading {
            Cell {
                cost: 0,
                start: usize::MAX,
            }
        } else {
            row[0].paid(1, Some(offset))
        };
        for (i, class) in classes.iter().enumerate() {
            let previous = row[i + 1];
            let cost = usize::from(id == 0 || !class.ids.contains(&id));
            let mut best = [
                diagonal.paid(cost, Some(offset)),
                row[i].paid(1, None),
                previous.paid(1, Some(offset)),
            ]
            .into_iter()
            .min_by_key(|cell| cell.rank())
            .unwrap();
            if class.loop_after && previous.rank() < best.rank() {
                best = previous;
            }
            row[i + 1] = best;
            diagonal = previous;
        }
        offset += width;
    };
    if profile == Profile::AsearchTsLib {
        for &unit in text {
            consume(u32::from(unit), 1);
        }
    } else {
        for value in char::decode_utf16(text.iter().copied()) {
            let (value, width) = value.map_or_else(
                |e| (u32::from(e.unpaired_surrogate()), 1),
                |c| (u32::from(c), c.len_utf16()),
            );
            consume(value, width);
        }
    }
    row[classes.len()].start
}

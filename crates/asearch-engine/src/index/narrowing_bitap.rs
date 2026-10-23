//! Compiles corpus-symbol classes into bounded scalar reachability masks.

use crate::transposed::workspace::Class;

const MAX_PATTERN: usize = 63;

#[derive(Debug)]
pub(super) struct Bitap {
    masks: [(u32, u64); MAX_PATTERN * 3],
    len: usize,
    length: usize,
    loops: u64,
}

impl Bitap {
    pub fn new() -> Self {
        Self {
            masks: [(0, 0); MAX_PATTERN * 3],
            len: 0,
            length: 0,
            loops: 0,
        }
    }

    /// Preserves host casing classes and persistent loops. Longer patterns
    /// require a separate accepting bit and use the cell-DP fallback. Resolve
    /// nonzero corpus IDs once when code-value masks avoid per-symbol corpus
    /// lookups; identity mapping retains ID masks for shorter candidates.
    pub fn compile(
        &mut self,
        classes: &[Class],
        leading: bool,
        symbol_value: impl Fn(u32) -> u32,
    ) -> bool {
        if classes.len() > MAX_PATTERN {
            return false;
        }
        self.len = 0;
        self.length = classes.len();
        self.loops = u64::from(leading);
        for (i, class) in classes.iter().enumerate() {
            if class.loop_after {
                self.loops |= 1 << (i + 1);
            }
            for id in class.ids {
                if id != 0 {
                    self.masks[self.len] = (symbol_value(id), 1 << i);
                    self.len += 1;
                }
            }
        }
        self.masks[..self.len].sort_unstable_by_key(|&(id, _)| id);
        let mut count = 0;
        for i in 0..self.len {
            let (id, mask) = self.masks[i];
            if count > 0 && self.masks[count - 1].0 == id {
                self.masks[count - 1].1 |= mask;
            } else {
                self.masks[count] = (id, mask);
                count += 1;
            }
        }
        self.len = count;
        true
    }

    /// Returns the first accepting edit level, or K+1 when no level accepts.
    pub fn level(&self, symbols: impl Iterator<Item = u32>, k: u8) -> u8 {
        let mut states = [0u64; 8];
        for (e, state) in states.iter_mut().enumerate().take(usize::from(k) + 1) {
            *state = (1 << (e.min(self.length) + 1)) - 1;
        }
        for symbol in symbols {
            let mask = self.masks[..self.len]
                .binary_search_by_key(&symbol, |&(value, _)| value)
                .map_or(0, |i| self.masks[i].1);
            for e in (1..=usize::from(k)).rev() {
                states[e] = (states[e] & self.loops)
                    | ((states[e] & mask) << 1)
                    | (states[e - 1] << 1)
                    | states[e - 1];
            }
            states[0] = (states[0] & self.loops) | ((states[0] & mask) << 1);
            for e in 1..=usize::from(k) {
                states[e] |= states[e - 1] << 1;
            }
        }
        (0..=k)
            .find(|&e| states[usize::from(e)] & (1 << self.length) != 0)
            .unwrap_or(k + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Independent minimum-cost matrix over corpus IDs, not state-word updates.
    fn oracle(classes: &[Class], leading: bool, text: &[u32]) -> usize {
        let mut rows = vec![vec![0usize; classes.len() + 1]; text.len() + 1];
        for (j, value) in rows[0].iter_mut().enumerate() {
            *value = j;
        }
        for (i, &id) in text.iter().enumerate() {
            rows[i + 1][0] = if leading { 0 } else { i + 1 };
            for (j, class) in classes.iter().enumerate() {
                let cost = usize::from(id == 0 || !class.ids.contains(&id));
                let mut value = (rows[i][j] + cost)
                    .min(rows[i + 1][j] + 1)
                    .min(rows[i][j + 1] + 1);
                if class.loop_after {
                    value = value.min(rows[i][j + 1]);
                }
                rows[i + 1][j + 1] = value;
            }
        }
        rows[text.len()][classes.len()]
    }

    #[test]
    fn resolved_masks_preserve_zero_surrogates_aliases_and_recompilation() {
        // Corpus IDs have a different order from these code values. ID zero
        // means absent, but code value zero is a valid input symbol.
        let alphabet = [0x1f600, 0, 0xd800, 0x41, 0x61];
        let resolve = |id: u32| alphabet[id as usize - 1];
        let mut kernel = Bitap::new();
        for ids in [[1, 0, 0], [2, 2, 0], [3, 0, 0], [4, 5, 4], [0, 0, 0]] {
            for leading in [false, true] {
                for loop_after in [false, true] {
                    for m in [0, 1, 31, 32, 62, 63] {
                        let classes = vec![Class { ids, loop_after }; m];
                        assert!(kernel.compile(&classes, leading, resolve));
                        let texts = [
                            vec![],
                            vec![0; m],
                            vec![1; m],
                            vec![2; m],
                            vec![3; m],
                            vec![4; m],
                            vec![4, 5, 4],
                            vec![0, 1, 2, 3, 4, 5, 0],
                        ];
                        for text in texts {
                            let expected = oracle(&classes, leading, &text);
                            let values: Vec<_> = text
                                .iter()
                                .map(|&id| if id == 0 { 0x10ffff } else { resolve(id) })
                                .collect();
                            for k in 0..=7 {
                                assert_eq!(
                                    usize::from(kernel.level(values.iter().copied(), k)),
                                    expected.min(usize::from(k) + 1)
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn class_masks_equal_cost_matrix_at_word_boundaries_and_all_budgets() {
        let mut kernel = Bitap::new();
        for m in [0, 1, 2, 7, 31, 32, 62, 63] {
            for leading in [false, true] {
                for looping in [false, true] {
                    let classes: Vec<_> = (0..m)
                        .map(|i| Class {
                            ids: [1 + (i % 3) as u32, 4, 4],
                            loop_after: looping && (i % 3 == 0 || i + 1 == m),
                        })
                        .collect();
                    assert!(kernel.compile(&classes, leading, |id| id));
                    let mut examples =
                        vec![vec![4; m], vec![4; m.saturating_sub(1)], vec![4; m + 1]];
                    if m > 0 {
                        let mut changed = vec![4; m];
                        changed[m / 2] = 5;
                        examples.push(changed);
                    }
                    for text in examples {
                        let expected = oracle(&classes, leading, &text);
                        for k in 0..=7 {
                            assert_eq!(
                                usize::from(kernel.level(text.iter().copied(), k)),
                                expected.min(usize::from(k) + 1)
                            );
                        }
                    }
                    for n in [0, 1, 3, 7, 32, 63, 70] {
                        for salt in 0..5 {
                            let text: Vec<_> = (0..n).map(|i| ((i + salt) % 5) as u32).collect();
                            let expected = oracle(&classes, leading, &text);
                            for k in 0..=7 {
                                assert_eq!(
                                    usize::from(kernel.level(text.iter().copied(), k)),
                                    expected.min(usize::from(k) + 1),
                                    "m={m}, n={n}, k={k}, leading={leading}, loops={looping}"
                                );
                            }
                        }
                    }
                }
            }
        }
        assert!(!kernel.compile(&vec![Class::default(); 64], false, |id| id));
        assert!(!kernel.compile(&vec![Class::default(); 65], false, |id| id));
    }
}

//! Complete text candidates, never the previous top-N. A conservative cost
//! comparison only uses narrowing when literal extension is proved safe.

use super::AsearchIndex;
use crate::{
    MatchMode, Profile, QueryWorkspace, SearchError,
    capacity::{Budget, add},
    transposed::workspace::{Class, TransposedWorkspace},
};
use std::mem::size_of;

#[derive(Debug)]
pub(super) struct Narrowing {
    history: Vec<Class>,
    candidates: Vec<usize>,
    previous: Vec<u8>,
    current: Vec<u8>,
    mode: MatchMode,
    leading: bool,
    valid: bool,
    k: u8,
    units: usize,
    bytes: usize,
    bitap: super::narrowing_bitap::Bitap,
}

impl Narrowing {
    pub fn new(
        index: &AsearchIndex,
        query: &QueryWorkspace,
        available: usize,
    ) -> Result<Self, SearchError> {
        let mut budget = Budget::new(size_of::<Self>(), available)?;
        let history = budget.reserve(query.pattern_capacity())?;
        let candidates = budget.reserve(index.text_count().min(candidate_budget(index)))?;
        let n = add(query.pattern_capacity(), 1)?;
        let mut previous = budget.reserve(n)?;
        previous.resize(n, 0);
        let mut current = budget.reserve(n)?;
        current.resize(n, 0);
        Ok(Self {
            history,
            candidates,
            previous,
            current,
            mode: MatchMode::Exact,
            leading: false,
            valid: false,
            k: 0,
            units: 0,
            bytes: budget.used,
            bitap: super::narrowing_bitap::Bitap::new(),
        })
    }

    pub fn capacity_bytes(&self) -> usize {
        self.bytes
    }

    pub fn reset(&mut self) {
        self.valid = false;
        self.candidates.clear();
        self.history.clear();
    }

    pub fn should_use(
        &self,
        index: &AsearchIndex,
        query: &QueryWorkspace,
        ws: &TransposedWorkspace,
    ) -> bool {
        let classes = ws.classes();
        if !self.valid
            || self.mode != query.options().mode
            || self.leading != leading(query)
            || query.max_ambig() > self.k
            || self.history.len() > classes.len()
            || !self
                .history
                .last()
                .map_or(self.leading, |class| class.loop_after)
        {
            return false;
        }
        if !self
            .history
            .iter()
            .zip(classes)
            .enumerate()
            .all(|(i, (old, new))| {
                old.ids == new.ids
                    && (i + 1 == self.history.len() || old.loop_after == new.loop_after)
            })
        {
            return false;
        }
        // Dense rows process 64 slots per word. This deliberately favors the
        // blocked/incremental path until the entire candidate set is tiny.
        self.units <= candidate_budget(index)
    }

    pub fn search(
        &mut self,
        index: &AsearchIndex,
        query: &QueryWorkspace,
        ws: &mut TransposedWorkspace,
    ) {
        let k = query.max_ambig();
        let cap = k + 1;
        let (classes, levels) = ws.narrowing_parts();
        let m = classes.len();
        let leading = leading(query);
        if self.candidates.is_empty() {
            ws.set_narrowing_stats(0);
            return;
        }
        // Resolve the corpus alphabet only when enough candidate text can
        // amortize query-mask translation. Short candidates keep ID masks.
        let compiled = if self.units >= m.saturating_mul(8) {
            self.search_bitap::<true>(index, query, classes, levels, leading)
        } else {
            self.search_bitap::<false>(index, query, classes, levels, leading)
        };
        if compiled {
            ws.set_narrowing_stats(self.candidates.len());
            return;
        }
        let mut evaluated = 0;
        for &text in &self.candidates {
            for i in 0..=m {
                self.previous[i] = (i.min(usize::from(cap))) as u8;
            }
            let input = index.text(text).expect("registered text ID");
            let mut consume = |symbol: u32| {
                self.current[0] = if leading {
                    0
                } else {
                    self.previous[0].saturating_add(1).min(cap)
                };
                let id = index.engine.symbol_id(symbol);
                for (j, class) in classes.iter().enumerate() {
                    let i = j + 1;
                    let cost = u8::from(id == 0 || !class.ids.contains(&id));
                    let mut value = self.previous[i - 1]
                        .saturating_add(cost)
                        .min(self.current[i - 1].saturating_add(1))
                        .min(self.previous[i].saturating_add(1))
                        .min(cap);
                    if class.loop_after {
                        value = value.min(self.previous[i]);
                    }
                    self.current[i] = value;
                }
                std::mem::swap(&mut self.previous, &mut self.current);
            };
            if query.options().profile == Profile::AsearchTsLib {
                for &unit in input {
                    consume(u32::from(unit));
                }
            } else {
                for symbol in std::char::decode_utf16(input.iter().copied()) {
                    consume(symbol.map_or_else(|e| u32::from(e.unpaired_surrogate()), u32::from));
                }
            }
            let level = self.previous[m];
            levels[text] = (level <= k).then_some(u16::from(level));
            evaluated += 1;
        }
        // All texts outside the previous complete set are already None.
        ws.set_narrowing_stats(evaluated);
    }

    /// Compiles and evaluates complete candidates with one fixed mask-key
    /// representation. Specialization removes representation branches from scans.
    fn search_bitap<const CODE_VALUES: bool>(
        &mut self,
        index: &AsearchIndex,
        query: &QueryWorkspace,
        classes: &[Class],
        levels: &mut [Option<u16>],
        leading: bool,
    ) -> bool {
        let k = query.max_ambig();
        if !self.bitap.compile(classes, leading, |id| {
            if CODE_VALUES {
                index.engine.symbol_value(id).expect("registered corpus ID")
            } else {
                id
            }
        }) {
            return false;
        }
        for &text in &self.candidates {
            let input = index.text(text).expect("registered text ID");
            let symbol = |value| {
                if CODE_VALUES {
                    value
                } else {
                    index.engine.symbol_id(value)
                }
            };
            let level = if query.options().profile == Profile::AsearchTsLib {
                self.bitap
                    .level(input.iter().map(|&unit| symbol(u32::from(unit))), k)
            } else {
                self.bitap.level(
                    std::char::decode_utf16(input.iter().copied()).map(|s| {
                        symbol(s.map_or_else(|e| u32::from(e.unpaired_surrogate()), u32::from))
                    }),
                    k,
                )
            };
            levels[text] = (level <= k).then_some(u16::from(level));
        }
        true
    }

    pub fn capture_candidate(&mut self, index: &AsearchIndex, text: usize) {
        if !self.valid {
            return;
        }
        let units = index.text(text).expect("validated text ID").len() + 1;
        let Some(total) = self
            .units
            .checked_add(units)
            .filter(|&n| n <= candidate_budget(index))
        else {
            // A prefix of the candidates is never a valid narrowing set. Once
            // its cost exceeds the dense budget, discard it and force dense
            // search next time; the next complete pass may become selective.
            self.valid = false;
            self.candidates.clear();
            return;
        };
        self.units = total;
        self.candidates.push(text);
    }

    pub fn begin_remember(&mut self, query: &QueryWorkspace, ws: &TransposedWorkspace) {
        self.candidates.clear();
        self.units = 0;
        self.history.clear();
        self.history.extend_from_slice(ws.classes());
        self.mode = query.options().mode;
        self.leading = leading(query);
        self.k = query.max_ambig();
        self.valid = self
            .history
            .last()
            .map_or(self.leading, |class| class.loop_after);
    }
}

fn leading(query: &QueryWorkspace) -> bool {
    query.options().mode == MatchMode::Include
        || (query.options().mode == MatchMode::WildcardSpace
            && query.pattern().first() == Some(&32))
}

// Scalar work is favored only for a tiny complete set. The weight is a
// conservative cost model, not a universal crossover or speed guarantee.
fn candidate_budget(index: &AsearchIndex) -> usize {
    index.engine.slots() / 64 / 8
}

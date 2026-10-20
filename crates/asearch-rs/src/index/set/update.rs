//! Candidate construction uses the old column's remaining budgets. Adoption
//! happens once, after complete format, uniqueness and liveness validation.
use super::{IndexSet, build::ARC_CONTROL};
use crate::{
    AsearchIndex, ExpectedSegment, ResourceLimits, SearchError,
    capacity::{Budget, add, mul},
};
use std::{
    mem::size_of,
    sync::{Arc, atomic::Ordering},
};

#[derive(Clone, Copy, Default)]
struct Counts {
    input: usize,
    texts: usize,
    units: usize,
    slots: usize,
}
impl Counts {
    fn include(&mut self, part: &AsearchIndex) -> Result<(), SearchError> {
        self.input = add(
            self.input,
            add(
                mul(
                    add(part.original.capacity(), part.normalized.capacity())?,
                    2,
                )?,
                mul(part.tombstones.capacity(), 4)?,
            )?,
        )?;
        self.texts = add(self.texts, part.text_count())?;
        self.units = add(self.units, part.normalized.len())?;
        self.slots = add(self.slots, part.engine.slots())?;
        Ok(())
    }
}
impl IndexSet {
    pub fn append_bytes(
        &mut self,
        bytes: Vec<u8>,
        expected: Option<ExpectedSegment>,
    ) -> Result<(), SearchError> {
        self.prepare_append(bytes, expected)?.commit();
        Ok(())
    }

    pub fn prepare_append(
        &mut self,
        bytes: Vec<u8>,
        expected: Option<ExpectedSegment>,
    ) -> Result<super::PreparedUpdate<'_>, SearchError> {
        let limits = self.limits;
        let candidate = self.load_candidate(vec![bytes], &[expected], true)?;
        Ok(super::PreparedUpdate {
            active: self,
            candidate,
            limits,
        })
    }

    pub fn replace_bytes(
        &mut self,
        sources: Vec<Vec<u8>>,
        expected: &[Option<ExpectedSegment>],
    ) -> Result<(), SearchError> {
        self.prepare_replace(sources, expected)?.commit();
        Ok(())
    }

    pub fn prepare_replace(
        &mut self,
        sources: Vec<Vec<u8>>,
        expected: &[Option<ExpectedSegment>],
    ) -> Result<super::PreparedUpdate<'_>, SearchError> {
        let limits = self.limits;
        let candidate = self.load_candidate(sources, expected, false)?;
        Ok(super::PreparedUpdate {
            active: self,
            candidate,
            limits,
        })
    }

    fn require_idle(&self) -> Result<(), SearchError> {
        if self.activity.load(Ordering::Relaxed) != 0 {
            return Err(SearchError::Busy(
                "drop set workspaces before adopting updates",
            ));
        }
        for part in &self.segments {
            if part.limits.max_working_bytes - part.available_working_bytes()?
                != part.capacity_bytes
            {
                return Err(SearchError::Busy(
                    "drop segment workspaces before adopting updates",
                ));
            }
        }
        Ok(())
    }

    fn load_candidate(
        &mut self,
        sources: Vec<Vec<u8>>,
        expected: &[Option<ExpectedSegment>],
        append: bool,
    ) -> Result<IndexSet, SearchError> {
        self.require_idle()?;
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(SearchError::ResourceLimit("index revision exhausted"))?;
        let count = if append {
            add(self.segments.len(), sources.len())?
        } else {
            sources.len()
        };
        if count > self.limits.max_segments {
            return Err(SearchError::ResourceLimit("server segment limit"));
        }
        if sources.is_empty() || sources.len() != expected.len() {
            return Err(SearchError::InvalidInput("invalid update source count"));
        }
        let mut old = Counts::default();
        for part in &self.segments {
            old.include(part)?;
        }
        let mut raw = 0;
        let mut texts = old.texts;
        let mut units = old.units;
        for source in &sources {
            if source.len() < 64 || &source[..4] != b"ASIX" {
                return Err(SearchError::InvalidIndex("invalid update header"));
            }
            raw = add(raw, source.capacity())?;
            texts = add(
                texts,
                u32::from_le_bytes(source[36..40].try_into().expect("header")) as usize,
            )?;
            units = add(
                units,
                u32::from_le_bytes(source[40..44].try_into().expect("header")) as usize,
            )?;
        }
        if add(old.input, raw)? > self.limits.max_total_bytes
            || texts > self.limits.max_texts
            || units > self.limits.max_units
        {
            return Err(SearchError::ResourceLimit(
                "old plus staged input count budget",
            ));
        }
        let old_working = self.limits.max_working_bytes - self.available_working_bytes()?;
        let outer = mul(sources.capacity(), size_of::<Vec<u8>>())?;
        let controls = mul(sources.len(), ARC_CONTROL)?;
        let mut budget = Budget::new(
            add(add(add(old_working, raw)?, outer)?, controls)?,
            self.limits.max_working_bytes,
        )?;
        let mut parts = budget.reserve(count)?;
        let metadata = add(
            add(outer, controls)?,
            mul(parts.capacity(), size_of::<Arc<AsearchIndex>>())?,
        )?;
        let options = self.segments[0].engine.options();
        let mut built = Counts::default();
        let mut built_working = 0;
        for (i, source) in sources.into_iter().enumerate() {
            raw -= source.capacity();
            let used = add(add(add(old_working, raw)?, metadata)?, built_working)?;
            let working = self
                .limits
                .max_working_bytes
                .checked_sub(used)
                .filter(|&n| n > 0)
                .ok_or(SearchError::ResourceLimit("update construction capacity"))?;
            let input = self
                .limits
                .max_total_bytes
                .checked_sub(add(add(old.input, built.input)?, raw)?)
                .filter(|&n| n > 0)
                .ok_or(SearchError::ResourceLimit(
                    "update input construction budget",
                ))?;
            let slots = self
                .limits
                .max_slots
                .checked_sub(add(old.slots, built.slots)?)
                .filter(|&n| n > 0)
                .ok_or(SearchError::ResourceLimit("update slot budget"))?;
            let limits = ResourceLimits {
                max_working_bytes: working,
                max_total_bytes: input,
                max_texts: (self.limits.max_texts - old.texts - built.texts).max(1),
                max_units: (self.limits.max_units - old.units - built.units).max(1),
                max_slots: slots,
                ..self.limits
            };
            let mut part = if let Some(expected) = &expected[i] {
                AsearchIndex::from_owned_bytes_expected(source, options, limits, expected)?
            } else {
                AsearchIndex::from_owned_bytes(source, options, limits)?
            };
            built_working = add(built_working, part.capacity_bytes())?;
            built.include(&part)?;
            part.set_runtime_limits(self.limits)?;
            parts.push(Arc::new(part));
        }
        if append {
            for part in &self.segments {
                parts.push(part.clone());
            }
            parts.rotate_right(self.segments.len());
        }
        self.candidate(parts, revision)
    }

    fn candidate(
        &mut self,
        parts: Vec<Arc<AsearchIndex>>,
        revision: u32,
    ) -> Result<IndexSet, SearchError> {
        let mut old_only = Counts::default();
        let mut overlap = 0;
        for old in &self.segments {
            if parts.iter().any(|part| Arc::ptr_eq(old, part)) {
                overlap = add(overlap, add(old.capacity_bytes(), ARC_CONTROL)?)?;
            } else {
                old_only.include(old)?;
            }
        }
        let reserved = *self
            .reservations
            .0
            .lock()
            .map_err(|_| SearchError::ResourceLimit("set accounting unavailable"))?;
        let outside = add(self.capacity_bytes - overlap, reserved)?;
        let working = self
            .limits
            .max_working_bytes
            .checked_sub(outside)
            .filter(|&n| n > 0)
            .ok_or(SearchError::ResourceLimit("update metadata peak budget"))?;
        let limits = ResourceLimits {
            max_working_bytes: working,
            max_total_bytes: self
                .limits
                .max_total_bytes
                .saturating_sub(old_only.input)
                .max(1),
            max_texts: self.limits.max_texts.saturating_sub(old_only.texts).max(1),
            max_units: self.limits.max_units.saturating_sub(old_only.units).max(1),
            max_slots: self.limits.max_slots.saturating_sub(old_only.slots).max(1),
            ..self.limits
        };
        let mut candidate = super::build::build(parts, limits)?;
        candidate.limits = ResourceLimits {
            max_working_bytes: self.limits.max_working_bytes - (self.capacity_bytes - overlap),
            ..self.limits
        };
        candidate.reservations = self.reservations.clone();
        candidate.activity = self.activity.clone();
        candidate.revision = revision;
        Ok(candidate)
    }
}

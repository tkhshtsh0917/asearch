//! Normalizes query updates transactionally into preallocated buffers.

use super::{ResourceLimits, SearchError};
use crate::{AmbigPolicy, MatchMode, Normalizer, Profile};
use std::mem::size_of;

#[derive(Clone, Copy, Debug)]
pub struct QueryOptions {
    pub normalizer: Normalizer,
    pub policy: AmbigPolicy,
    pub mode: MatchMode,
    pub profile: Profile,
    pub wrap: bool,
    /// Break equal edit distances by the earliest normalized UTF-16 match start.
    pub rank_by_position: bool,
}

impl Default for QueryOptions {
    fn default() -> Self {
        Self {
            normalizer: Normalizer::default(),
            policy: AmbigPolicy::default(),
            mode: MatchMode::WildcardSpace,
            profile: Profile::Ideal,
            wrap: true,
            rank_by_position: false,
        }
    }
}

/// Two reserved UTF-16 buffers keep the successful query unchanged on error.
/// Normalization and policy evaluation run before internal wrap is added.
///
/// ```
/// use asearch_engine::{QueryOptions, QueryWorkspace, ResourceLimits};
///
/// let mut query = QueryWorkspace::new(QueryOptions::default(), ResourceLimits::default())?;
/// let input: Vec<_> = "ｶﾞｷｸｹ".encode_utf16().collect();
/// query.update(&input)?;
/// assert_eq!(query.max_ambig(), 1);
/// assert_eq!(String::from_utf16(query.normalized()).unwrap(), "がきくけ");
/// assert_eq!(String::from_utf16(query.pattern()).unwrap(), " がきくけ ");
/// # Ok::<(), asearch_engine::SearchError>(())
/// ```
#[derive(Debug)]
pub struct QueryWorkspace {
    options: QueryOptions,
    active: Vec<u16>,
    staging: Vec<u16>,
    max_units: usize,
    input_units: usize,
    level: u8,
    wrapped: bool,
    capacity_bytes: usize,
}

fn bytes(a: usize, b: usize, limit: usize) -> Result<usize, SearchError> {
    a.checked_add(b)
        .and_then(|count| count.checked_mul(size_of::<u16>()))
        .and_then(|bytes| bytes.checked_add(size_of::<QueryWorkspace>()))
        .filter(|&bytes| bytes <= limit)
        .ok_or(SearchError::ResourceLimit(
            "query workspace exceeds capacity budget",
        ))
}

impl QueryWorkspace {
    pub fn new(options: QueryOptions, limits: ResourceLimits) -> Result<Self, SearchError> {
        limits.validate()?;
        let count = limits
            .max_pattern_units
            .checked_add(2)
            .ok_or(SearchError::ResourceLimit("query capacity overflow"))?;
        bytes(count, count, limits.max_working_bytes)?;
        let mut active = Vec::new();
        active
            .try_reserve_exact(count)
            .map_err(|_| SearchError::ResourceLimit("query reservation failed"))?;
        bytes(active.capacity(), count, limits.max_working_bytes)?;
        let mut staging = Vec::new();
        staging
            .try_reserve_exact(count)
            .map_err(|_| SearchError::ResourceLimit("query reservation failed"))?;
        let capacity_bytes = bytes(
            active.capacity(),
            staging.capacity(),
            limits.max_working_bytes,
        )?;
        let mut workspace = Self {
            options,
            active,
            staging,
            max_units: limits.max_pattern_units,
            input_units: 0,
            level: 0,
            wrapped: options.mode == MatchMode::WildcardSpace && options.wrap,
            capacity_bytes,
        };
        workspace.update(&[])?;
        Ok(workspace)
    }

    pub fn update(&mut self, input: &[u16]) -> Result<(), SearchError> {
        self.update_with_options(input, self.options)
    }

    /// Reuses both buffers when stateless searches change their options.
    /// The descriptor and previous successful query change only on success.
    pub fn update_with_options(
        &mut self,
        input: &[u16],
        options: QueryOptions,
    ) -> Result<(), SearchError> {
        if input.len() > self.max_units {
            return Err(SearchError::ResourceLimit(
                "query exceeds UTF-16 unit limit",
            ));
        }
        options
            .normalizer
            .normalize_into(input, &mut self.staging)
            .map_err(|_| {
                SearchError::ResourceLimit("query normalization capacity is insufficient")
            })?;
        let level = options
            .policy
            .evaluate(&self.staging, options.mode, options.profile);
        let wrapped = options.mode == MatchMode::WildcardSpace && options.wrap;
        if wrapped {
            self.staging.insert(0, 0x20);
            self.staging.push(0x20);
        }
        std::mem::swap(&mut self.active, &mut self.staging);
        self.level = level;
        self.options = options;
        self.wrapped = wrapped;
        self.input_units = input.len();
        Ok(())
    }

    /// Selects a validated policy from host-side context without normalizing
    /// the input again or changing the prepared pattern.
    pub fn set_policy(&mut self, policy: AmbigPolicy) {
        self.level = policy.evaluate(self.normalized(), self.options.mode, self.options.profile);
        self.options.policy = policy;
    }

    pub fn normalized(&self) -> &[u16] {
        if self.wrapped {
            &self.active[1..self.active.len() - 1]
        } else {
            &self.active
        }
    }

    pub fn pattern(&self) -> &[u16] {
        &self.active
    }

    pub fn max_ambig(&self) -> u8 {
        self.level
    }

    pub fn capacity_bytes(&self) -> usize {
        self.capacity_bytes
    }

    pub fn options(&self) -> QueryOptions {
        self.options
    }

    /// Raw input length, before normalization can contract kana pairs.
    pub fn input_units(&self) -> usize {
        self.input_units
    }

    pub fn pattern_capacity(&self) -> usize {
        self.max_units + 2
    }
}

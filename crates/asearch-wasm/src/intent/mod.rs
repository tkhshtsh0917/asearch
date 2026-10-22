//! Owns WASM intent handles and validates flat expression inputs before compilation.

mod query;
use crate::index::{config, error, references::References};
use asearch_engine::{
    IntentEntry, IntentIndex, QueryOptions, QueryWorkspace, ResourceLimits, SearchError,
    TransposedOptions, WorkingReservation,
};
use std::mem::size_of;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct IntentHandle {
    index: IntentIndex,
    query: QueryWorkspace,
    refs: References,
    options: TransposedOptions,
    _lease: WorkingReservation,
}
#[wasm_bindgen]
impl IntentHandle {
    pub fn check_transport(&self, bytes: u32) -> Result<(), JsError> {
        let result = self.index.reserve_working(bytes as usize);
        result.map(|_| ()).map_err(error)
    }

    pub fn from_flat(
        units: Vec<u16>,
        lengths: Vec<u32>,
        keys: Vec<u32>,
        orders: Vec<u32>,
        config: &[u32],
    ) -> Result<IntentHandle, JsError> {
        let result = (|| -> Result<Self, SearchError> {
            let (options, limits) = config::index_config(config)?;
            if lengths.len() != keys.len()
                || keys.len() != orders.len()
                || keys.len() > limits.max_texts
            {
                return Err(SearchError::InvalidInput("invalid flat intent entries"));
            }
            let input = units
                .len()
                .checked_mul(2)
                .and_then(|n| {
                    keys.len()
                        .checked_mul(size_of::<IntentEntry<'_>>() + 12)
                        .and_then(|metadata| n.checked_add(metadata))
                })
                .ok_or(SearchError::ResourceLimit("intent input overflow"))?;
            if input > limits.max_working_bytes {
                return Err(SearchError::ResourceLimit("intent input capacity"));
            }
            let mut entries = Vec::new();
            entries
                .try_reserve_exact(keys.len())
                .map_err(|_| SearchError::ResourceLimit("intent entry reservation"))?;
            if entries.capacity() != keys.len() {
                return Err(SearchError::ResourceLimit("intent entry capacity"));
            }
            let mut offset = 0usize;
            for i in 0..keys.len() {
                let end = offset
                    .checked_add(lengths[i] as usize)
                    .filter(|&end| end <= units.len())
                    .ok_or(SearchError::InvalidInput("invalid intent offsets"))?;
                entries.push(IntentEntry {
                    key: keys[i],
                    order: orders[i],
                    expr: &units[offset..end],
                });
                offset = end;
            }
            if offset != units.len() {
                return Err(SearchError::InvalidInput("extra intent units"));
            }
            drop(lengths);
            drop(keys);
            drop(orders);
            let index = IntentIndex::from_entries(&entries, options, limits)?;
            drop(entries);
            drop(units);
            let query = QueryWorkspace::new(
                QueryOptions {
                    normalizer: options.normalizer,
                    profile: options.profile,
                    ..QueryOptions::default()
                },
                ResourceLimits {
                    max_working_bytes: index.available_working_bytes()?,
                    ..limits
                },
            )?;
            let lease = index.reserve_working(
                query
                    .capacity_bytes()
                    .checked_add(size_of::<Self>())
                    .ok_or(SearchError::ResourceLimit("intent handle capacity"))?,
            )?;
            let refs = References::for_intent(&index)?;
            Ok(Self {
                index,
                query,
                refs,
                options,
                _lease: lease,
            })
        })();
        result.map_err(error)
    }

    pub fn search_prepared(
        &mut self,
        keys: &[u32],
        limit: u32,
        count_total: bool,
    ) -> Result<Vec<u32>, JsError> {
        let result = (|| -> Result<Vec<u32>, SearchError> {
            let _input = self.index.reserve_working(
                keys.len()
                    .checked_mul(4)
                    .ok_or(SearchError::ResourceLimit("casing input capacity"))?,
            )?;
            let result = self
                .index
                .search(&self.query, Some(keys), limit as usize, count_total)?;
            let _result = self.index.reserve_working(result.capacity_bytes())?;
            let totals = if count_total {
                usize::from(self.query.max_ambig()) + 1
            } else {
                0
            };
            let count = result
                .hits
                .len()
                .checked_mul(4)
                .and_then(|n| n.checked_add(8 + totals))
                .ok_or(SearchError::ResourceLimit("intent result overflow"))?;
            let _packed = self.index.reserve_working(
                count
                    .checked_mul(4)
                    .ok_or(SearchError::ResourceLimit("intent result overflow"))?,
            )?;
            let mut packed = Vec::new();
            packed
                .try_reserve_exact(count)
                .map_err(|_| SearchError::ResourceLimit("intent result reservation"))?;
            if packed.capacity() != count {
                return Err(SearchError::ResourceLimit("intent result capacity"));
            }
            packed.extend_from_slice(&[result.hits.len() as u32, totals as u32, 0, 0, 0, 4, 0, 0]);
            for hit in result.hits {
                packed.extend_from_slice(&[
                    hit.key,
                    hit.order,
                    self.refs.get(hit.entry)?,
                    u32::from(hit.level),
                ]);
            }
            if let Some(values) = result.totals {
                packed.extend(values[..totals].iter().map(|&n| n as u32));
            }
            Ok(packed)
        })();
        result.map_err(error)
    }

    pub fn explain_prepared(
        &self,
        reference: u32,
        keys: &[u32],
        original: &[u16],
        supplied: bool,
    ) -> Result<Vec<u32>, JsError> {
        let result = (|| -> Result<Vec<u32>, SearchError> {
            if supplied || !original.is_empty() {
                return Err(SearchError::InvalidInput(
                    "IntentIndex does not accept original overrides",
                ));
            }
            let _input = self.index.reserve_working(
                keys.len()
                    .checked_mul(4)
                    .ok_or(SearchError::ResourceLimit("casing input capacity"))?,
            )?;
            let entry = self.refs.resolve(reference)?;
            let Some(explanation) = self.index.explain(entry, &self.query, Some(keys))? else {
                return Ok(Vec::new());
            };
            crate::index::explain::pack(explanation, |bytes| self.index.reserve_working(bytes))
        })();
        result.map_err(error)
    }
}

//! Owns shared segment state and exposes read-only index handles to JS.

pub(crate) mod config;
mod expected;
pub(crate) mod explain;
pub(crate) mod references;
mod search;
mod session;
mod stream;
mod update;

use asearch_engine::{
    AsearchIndex, IndexSet, QueryOptions, QueryWorkspace, ResourceLimits, SearchError,
    SetWorkspace, TransposedOptions, WorkingReservation,
};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::prelude::*;

pub(super) fn error(error: SearchError) -> JsError {
    JsError::new(&format!("{}: {error}", error.code()))
}
pub(super) struct Shared {
    index: IndexSet,
    refs: references::References,
    busy: bool,
}

#[wasm_bindgen]
pub struct IndexHandle {
    index: Rc<RefCell<Shared>>,
    query: QueryWorkspace,
    workspace: Option<SetWorkspace>,
    options: TransposedOptions,
    limits: ResourceLimits,
    cache: Option<(usize, u8)>,
    _query_lease: WorkingReservation,
}

impl IndexHandle {
    fn wrap(
        index: AsearchIndex,
        options: TransposedOptions,
        limits: ResourceLimits,
    ) -> Result<Self, JsError> {
        let build = || -> Result<Self, SearchError> {
            let index = IndexSet::new(vec![index], limits)?;
            let refs = references::References::new(&index)?;
            Self::shared(
                Rc::new(RefCell::new(Shared {
                    index,
                    refs,
                    busy: false,
                })),
                options,
                limits,
            )
        };
        build().map_err(error)
    }

    fn live_bytes(&self) -> Result<usize, SearchError> {
        Ok(self.limits.max_working_bytes - self.index.borrow().index.available_working_bytes()?)
    }

    fn shared(
        index: Rc<RefCell<Shared>>,
        options: TransposedOptions,
        limits: ResourceLimits,
    ) -> Result<Self, SearchError> {
        let remaining = index.borrow().index.available_working_bytes()?;
        let query = QueryWorkspace::new(
            QueryOptions {
                normalizer: options.normalizer,
                profile: options.profile,
                ..QueryOptions::default()
            },
            ResourceLimits {
                max_working_bytes: remaining,
                ..limits
            },
        )?;
        let lease = index.borrow().index.reserve_working(
            query
                .capacity_bytes()
                .checked_add(
                    std::mem::size_of::<Self>()
                        + std::mem::size_of::<Shared>()
                        + 2 * std::mem::size_of::<usize>(),
                )
                .ok_or(SearchError::ResourceLimit("handle capacity overflow"))?,
        )?;
        Ok(Self {
            index,
            query,
            workspace: None,
            options,
            limits,
            cache: None,
            _query_lease: lease,
        })
    }

    fn ensure_workspace(&mut self) -> Result<(), SearchError> {
        if self.workspace.is_none() {
            let shared = self.index.borrow();
            let mut ws = shared
                .index
                .workspace(&self.query, self.cache.map_or(7, |(_, k)| k))?;
            if let Some((cache, _)) = self.cache {
                let packed = shared
                    .index
                    .document_count()
                    .checked_mul(16)
                    .and_then(|n| n.checked_add(64))
                    .ok_or(SearchError::ResourceLimit("result capacity overflow"))?;
                ws.enable_planner(
                    &shared.index,
                    &self.query,
                    cache,
                    packed
                        .checked_add(self.query.pattern_capacity().checked_mul(12).ok_or(
                            SearchError::ResourceLimit("query transport capacity overflow"),
                        )?)
                        .ok_or(SearchError::ResourceLimit(
                            "session transient capacity overflow",
                        ))?,
                )?;
            }
            self.workspace = Some(ws);
        }
        Ok(())
    }
}

#[wasm_bindgen]
impl IndexHandle {
    pub fn check_transport(&self, bytes: u32) -> Result<(), JsError> {
        let result = self.index.borrow().index.reserve_working(bytes as usize);
        result.map(|_| ()).map_err(error)
    }

    pub fn from_bytes(
        bytes: Vec<u8>,
        config: &[u32],
        expected: &[u8],
    ) -> Result<IndexHandle, JsError> {
        let (options, limits) = config::index_config(config).map_err(error)?;
        let limits = ResourceLimits {
            max_working_bytes: limits
                .max_working_bytes
                .checked_sub(expected.len())
                .filter(|&n| n > 0)
                .ok_or_else(|| error(SearchError::ResourceLimit("expected transport budget")))?,
            ..limits
        };
        Self::wrap(
            expected::load(bytes, options, limits, expected).map_err(error)?,
            options,
            limits,
        )
    }
}

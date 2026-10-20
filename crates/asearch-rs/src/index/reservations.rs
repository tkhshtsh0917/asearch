//! Shared accounting for simultaneously retained workspaces. Each lease also
//! reserves its query buffers, conservatively counting a shared query twice.
use crate::{SearchError, capacity::add};
use std::sync::{Arc, Mutex};

#[derive(Debug, Default)]
pub(super) struct Reservations(pub Mutex<usize>);

pub(super) const OVERHEAD: usize =
    std::mem::size_of::<Reservations>() + 2 * std::mem::size_of::<usize>();

#[derive(Debug)]
pub(super) struct Lease {
    owner: Arc<Reservations>,
    bytes: usize,
}

impl Lease {
    pub fn new(owner: Arc<Reservations>, bytes: usize) -> Self {
        Self { owner, bytes }
    }

    pub fn owns(&self, owner: &Arc<Reservations>) -> bool {
        Arc::ptr_eq(&self.owner, owner)
    }

    pub fn validate_query(&self, bytes: usize, workspace: usize) -> Result<(), SearchError> {
        if add(bytes, workspace)? > self.bytes {
            return Err(SearchError::ResourceLimit(
                "query exceeds workspace reservation",
            ));
        }
        Ok(())
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        let mut used = self
            .owner
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        *used -= self.bytes;
    }
}

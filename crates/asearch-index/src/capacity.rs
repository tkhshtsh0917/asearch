//! Reserves vectors with checked arithmetic and accounts for their actual capacity.

use crate::SearchError;
use std::mem::size_of;

pub fn add(a: usize, b: usize) -> Result<usize, SearchError> {
    a.checked_add(b)
        .ok_or(SearchError::ResourceLimit("search capacity overflow"))
}

pub fn mul(a: usize, b: usize) -> Result<usize, SearchError> {
    a.checked_mul(b)
        .ok_or(SearchError::ResourceLimit("search capacity overflow"))
}

/// Tracks live allocation capacities, including build staging. All large
/// reservations are checked before allocation and again for actual capacity.
pub struct Budget {
    pub used: usize,
    limit: usize,
}

impl Budget {
    pub fn new(used: usize, limit: usize) -> Result<Self, SearchError> {
        if used > limit {
            return Err(SearchError::ResourceLimit("search capacity budget"));
        }
        Ok(Self { used, limit })
    }

    pub fn reserve<T>(&mut self, count: usize) -> Result<Vec<T>, SearchError> {
        let requested = add(self.used, mul(count, size_of::<T>())?)?;
        if requested > self.limit {
            return Err(SearchError::ResourceLimit("search capacity budget"));
        }
        let mut result = Vec::new();
        result
            .try_reserve_exact(count)
            .map_err(|_| SearchError::ResourceLimit("search reservation failed"))?;
        let actual = add(self.used, mul(result.capacity(), size_of::<T>())?)?;
        if actual > self.limit {
            return Err(SearchError::ResourceLimit("search capacity budget"));
        }
        self.used = actual;
        Ok(result)
    }

    pub fn release<T>(&mut self, buffer: Vec<T>) {
        self.used -= buffer.capacity() * size_of::<T>();
        drop(buffer);
    }
}

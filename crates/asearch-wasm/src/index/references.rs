//! Opaque references never alias another root or a later successful update.
use asearch_rs::{IndexSet, SearchError, WorkingReservation};
use std::{
    mem::size_of,
    sync::atomic::{AtomicU32, Ordering},
};

static NEXT: AtomicU32 = AtomicU32::new(1);
pub(super) struct References {
    by_text: Vec<u32>,
    reverse: Vec<(u32, usize)>,
    _leases: [Option<WorkingReservation>; 2],
}
impl References {
    pub fn new(index: &IndexSet) -> Result<Self, SearchError> {
        Self::allocate(index.text_count(), |bytes| index.reserve_working(bytes))
    }
    fn allocate(
        count: usize,
        reserve: impl Fn(usize) -> Result<WorkingReservation, SearchError>,
    ) -> Result<Self, SearchError> {
        let bytes = count
            .checked_mul(size_of::<u32>() + size_of::<(u32, usize)>())
            .and_then(|n| n.checked_add(size_of::<Self>()))
            .ok_or(SearchError::ResourceLimit("reference capacity overflow"))?;
        let lease = reserve(bytes)?;
        let mut by_text = Vec::new();
        let mut reverse = Vec::new();
        by_text
            .try_reserve_exact(count)
            .map_err(|_| SearchError::ResourceLimit("reference reservation"))?;
        reverse
            .try_reserve_exact(count)
            .map_err(|_| SearchError::ResourceLimit("reference reservation"))?;
        let actual = by_text.capacity() * size_of::<u32>()
            + reverse.capacity() * size_of::<(u32, usize)>()
            + size_of::<Self>();
        let extra = if actual > bytes {
            Some(reserve(actual - bytes)?)
        } else {
            None
        };
        by_text.resize(count, 0);
        Ok(Self {
            by_text,
            reverse,
            _leases: [Some(lease), extra],
        })
    }
    pub fn get(&mut self, flat: usize) -> Result<u32, SearchError> {
        let slot = self
            .by_text
            .get_mut(flat)
            .ok_or(SearchError::InvalidInput("invalid text reference"))?;
        if *slot == 0 {
            let id = NEXT
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
                .map_err(|_| SearchError::ResourceLimit("reference number space exhausted"))?;
            *slot = id;
            self.reverse.push((id, flat));
        }
        Ok(*slot)
    }
}

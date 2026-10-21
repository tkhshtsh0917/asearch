mod load;
#[cfg(feature = "builder")]
mod write;
#[cfg(feature = "builder")]
pub(crate) use asearch_builder as encoding;
use asearch_index::{u16_at, view::View};
pub(super) use load::load;
#[cfg(feature = "builder")]
pub(super) use write::{size, write, write_limited};
fn invalid() -> crate::SearchError {
    crate::SearchError::InvalidIndex("invalid ASIX segment")
}

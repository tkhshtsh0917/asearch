use std::{error::Error, fmt, mem::size_of};

/// Capacity limits for one scalar matcher and one reserved workspace.
/// They do not bound process memory or allocations owned by the caller.
#[derive(Clone, Copy, Debug)]
pub struct ScalarLimits {
    pub max_pattern_units: usize,
    pub max_working_bytes: usize,
}

impl Default for ScalarLimits {
    fn default() -> Self {
        Self {
            max_pattern_units: 65_535,
            max_working_bytes: 512 * 1024 * 1024,
        }
    }
}

impl ScalarLimits {
    pub(super) fn validate(self) -> Result<(), ScalarError> {
        if self.max_pattern_units == 0 || self.max_working_bytes == 0 {
            Err(ScalarError::InvalidLimits)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarError {
    InvalidLimits,
    PatternLimit,
    WordCapacity,
    WorkingLimit,
    Reservation,
    WorkspaceCapacity,
}

impl fmt::Display for ScalarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidLimits => "scalar limits must be positive",
            Self::PatternLimit => "pattern exceeds the UTF-16 unit limit",
            Self::WordCapacity => "pattern needs a larger state word, including the start position",
            Self::WorkingLimit => "scalar working capacity exceeds the byte budget",
            Self::Reservation => "scalar memory reservation failed",
            Self::WorkspaceCapacity => "workspace does not have enough corpus output slots",
        })
    }
}
impl Error for ScalarError {}

pub(super) fn checked_bytes<T>(
    count: usize,
    base: usize,
    limit: usize,
) -> Result<usize, ScalarError> {
    count
        .checked_mul(size_of::<T>())
        .and_then(|bytes| base.checked_add(bytes))
        .filter(|&bytes| bytes <= limit)
        .ok_or(ScalarError::WorkingLimit)
}

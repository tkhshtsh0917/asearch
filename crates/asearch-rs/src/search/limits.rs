use std::{error::Error, fmt};

/// Index capacity limits. The owning index will account for all of its
/// segments and sessions; standalone query workspaces account for themselves.
#[derive(Clone, Copy, Debug)]
pub struct ResourceLimits {
    pub max_total_bytes: usize,
    pub max_texts: usize,
    pub max_units: usize,
    pub max_slots: usize,
    pub max_working_bytes: usize,
    pub max_pattern_units: usize,
    pub max_segments: usize,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_total_bytes: 256 * 1024 * 1024,
            max_texts: 1_000_000,
            max_units: 10_000_000,
            max_slots: 16_777_216,
            max_working_bytes: 512 * 1024 * 1024,
            max_pattern_units: 65_535,
            max_segments: 8,
        }
    }
}

impl ResourceLimits {
    pub fn validate(self) -> Result<(), SearchError> {
        let values = [
            self.max_total_bytes,
            self.max_texts,
            self.max_units,
            self.max_slots,
            self.max_working_bytes,
            self.max_pattern_units,
            self.max_segments,
        ];
        if values
            .iter()
            .any(|&value| value == 0 || value as u128 > 9_007_199_254_740_991)
            || self.max_segments > 8
        {
            return Err(SearchError::InvalidInput(
                "limits require positive safe integers and at most eight segments",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchError {
    InvalidInput(&'static str),
    InvalidIndex(&'static str),
    ResourceLimit(&'static str),
}

impl SearchError {
    pub fn code(self) -> &'static str {
        match self {
            Self::InvalidInput(_) => "INVALID_INPUT",
            Self::InvalidIndex(_) => "INVALID_INDEX",
            Self::ResourceLimit(_) => "RESOURCE_LIMIT",
        }
    }
}

impl fmt::Display for SearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message)
            | Self::InvalidIndex(message)
            | Self::ResourceLimit(message) => f.write_str(message),
        }
    }
}
impl Error for SearchError {}

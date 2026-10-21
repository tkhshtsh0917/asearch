//! Versioned, deliberately limited Japanese normalization over UTF-16 units.

mod tables;
mod transform;

use std::{error::Error, fmt};

/// Validated normalization descriptor. None and ja-basic both have version 1.
///
/// ```
/// use asearch_index::Normalizer;
///
/// let input: Vec<u16> = "ＡＢＣ ｶﾞ".encode_utf16().collect();
/// let mut output = Vec::with_capacity(input.len());
/// Normalizer::JA_BASIC.normalize_into(&input, &mut output)?;
/// assert_eq!(String::from_utf16(&output).unwrap(), "abc が");
/// # Ok::<(), asearch_index::NormalizeError>(())
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Normalizer {
    id: u16,
    options: u32,
}

impl Default for Normalizer {
    fn default() -> Self {
        Self::JA_BASIC
    }
}

impl Normalizer {
    pub const NONE: Self = Self { id: 0, options: 0 };
    /// Items 1–5 enabled; dash folding (item 6) disabled.
    pub const JA_BASIC: Self = Self { id: 1, options: 31 };

    pub fn from_descriptor(id: u16, version: u16, options: u32) -> Result<Self, NormalizeError> {
        if id > 1 {
            return Err(NormalizeError::UnsupportedId);
        }
        if version != 1 {
            return Err(NormalizeError::UnsupportedVersion);
        }
        if (id == 0 && options != 0) || options > 63 {
            return Err(NormalizeError::InvalidOptions);
        }
        Ok(Self { id, options })
    }

    pub fn descriptor(self) -> (u16, u16, u32) {
        (self.id, 1, self.options)
    }

    /// Recomputes original UTF-16 ranges for display, including composed kana.
    /// Both outputs need capacity for the input unit count; errors preserve them.
    pub fn normalize_mapped_into(
        self,
        input: &[u16],
        output: &mut Vec<u16>,
        ranges: &mut Vec<(usize, usize)>,
    ) -> Result<(), NormalizeError> {
        if output.capacity() < input.len() || ranges.capacity() < input.len() {
            return Err(NormalizeError::NeedsCapacity {
                required: input.len(),
            });
        }
        output.clear();
        ranges.clear();
        if self.id == 0 {
            for (i, &unit) in input.iter().enumerate() {
                output.push(unit);
                ranges.push((i, i + 1));
            }
        } else {
            transform::mapped(input, self.options, |unit, start, end| {
                output.push(unit);
                ranges.push((start, end));
            });
        }
        Ok(())
    }

    /// Writes into an already reserved buffer without allocating. Capacity
    /// must cover the input unit count (the transform never expands it).
    /// An error preserves the previous output, including its length.
    pub fn normalize_into(
        self,
        input: &[u16],
        output: &mut Vec<u16>,
    ) -> Result<(), NormalizeError> {
        if output.capacity() < input.len() {
            return Err(NormalizeError::NeedsCapacity {
                required: input.len(),
            });
        }
        output.clear();
        if self.id == 0 {
            output.extend_from_slice(input);
        } else {
            transform::normalize(input, output, self.options);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NormalizeError {
    UnsupportedId,
    UnsupportedVersion,
    InvalidOptions,
    NeedsCapacity { required: usize },
}

impl fmt::Display for NormalizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedId => f.write_str("unsupported normalizer id"),
            Self::UnsupportedVersion => f.write_str("unsupported normalizer version"),
            Self::InvalidOptions => f.write_str("invalid normalizer options"),
            Self::NeedsCapacity { required } => write!(
                f,
                "normalizer requires capacity for {required} UTF-16 units"
            ),
        }
    }
}
impl Error for NormalizeError {}

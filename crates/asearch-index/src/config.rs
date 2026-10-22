//! Decodes flat host configuration and deducts its transport storage from working limits.

use crate::{Normalizer, Profile, ResourceLimits, SearchError, TransposedOptions};
/// Decodes twelve host words and reserves configuration transport within the working budget.
pub fn index_config(config: &[u32]) -> Result<(TransposedOptions, ResourceLimits), SearchError> {
    if config.len() != 12 {
        return Err(SearchError::InvalidInput("invalid index config"));
    }
    let profile = match config[0] {
        0 => Profile::AsearchTs,
        1 => Profile::AsearchTsLib,
        2 => Profile::Ideal,
        _ => return Err(SearchError::InvalidInput("unknown profile")),
    };
    let id = u16::try_from(config[1])
        .map_err(|_| SearchError::InvalidInput("normalizer id overflow"))?;
    let version = u16::try_from(config[2])
        .map_err(|_| SearchError::InvalidInput("normalizer version overflow"))?;
    let normalizer = Normalizer::from_descriptor(id, version, config[3])
        .map_err(|_| SearchError::InvalidInput("invalid normalizer"))?;
    let limits = ResourceLimits {
        max_total_bytes: config[5] as usize,
        max_texts: config[6] as usize,
        max_units: config[7] as usize,
        max_slots: config[8] as usize,
        max_working_bytes: (config[9] as usize)
            .checked_sub(config.len() * 4)
            .filter(|&n| n > 0)
            .ok_or(SearchError::ResourceLimit("index config transport budget"))?,
        max_pattern_units: config[10] as usize,
        max_segments: config[11] as usize,
    };
    limits.validate()?;
    Ok((
        TransposedOptions {
            profile,
            normalizer,
            block_words: config[4] as usize,
        },
        limits,
    ))
}

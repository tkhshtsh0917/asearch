//! Maps host configuration words to validated shared index descriptors.

use asearch_engine::{
    AmbigPolicy, LengthUnit, MatchMode, QueryOptions, SearchError, TransposedOptions,
};
pub(crate) use asearch_index::index_config;

pub(crate) fn query_config(
    base: TransposedOptions,
    config: &[u32],
) -> Result<QueryOptions, SearchError> {
    if config.len() != 7 || config[1] > 1 || config[6] > 1 {
        return Err(SearchError::InvalidInput("invalid search config"));
    }
    let mode = match config[0] {
        0 => MatchMode::Exact,
        1 => MatchMode::Include,
        2 => MatchMode::WildcardSpace,
        _ => return Err(SearchError::InvalidInput("unknown mode")),
    };
    let k = u8::try_from(config[3]).map_err(|_| SearchError::InvalidInput("invalid policy max"))?;
    let policy = match config[2] {
        0 => AmbigPolicy::fixed(k),
        1 => {
            let unit = match config[5] {
                0 => LengthUnit::Profile,
                1 => LengthUnit::CodePoint,
                2 => LengthUnit::Utf16,
                _ => return Err(SearchError::InvalidInput("unknown length unit")),
            };
            AmbigPolicy::by_length(config[4] as usize, k, unit, config[6] != 0)
        }
        _ => return Err(SearchError::InvalidInput("unknown policy")),
    }
    .map_err(|_| SearchError::InvalidInput("invalid ambiguity policy"))?;
    Ok(QueryOptions {
        normalizer: base.normalizer,
        profile: base.profile,
        mode,
        wrap: config[1] != 0,
        policy,
    })
}

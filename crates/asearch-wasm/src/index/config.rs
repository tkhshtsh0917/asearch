//! Maps host configuration words to validated shared index descriptors.

use asearch_engine::{
    AmbigPolicy, LengthUnit, MatchMode, QueryOptions, SearchError, TransposedOptions,
};
pub(crate) use asearch_index::index_config;

pub(crate) fn query_config(
    base: TransposedOptions,
    config: &[u32],
) -> Result<QueryOptions, SearchError> {
    if !(7..=8).contains(&config.len())
        || config[1] > 1
        || config[6] > 1
        || config.get(7).is_some_and(|&v| v > 1)
    {
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
        rank_by_position: config.get(7) == Some(&1),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positional_config_extends_but_does_not_reinterpret_the_legacy_abi() {
        let base = TransposedOptions::default();
        let legacy = [2, 1, 0, 0, 1, 0, 0];
        assert!(!query_config(base, &legacy).unwrap().rank_by_position);
        let mut extended = legacy.to_vec();
        extended.push(0);
        assert!(!query_config(base, &extended).unwrap().rank_by_position);
        extended[7] = 1;
        assert!(query_config(base, &extended).unwrap().rank_by_position);
        extended[7] = 2;
        assert!(query_config(base, &extended).is_err());
        extended.push(0);
        assert!(query_config(base, &extended).is_err());
        assert!(query_config(base, &[]).is_err());
    }
}

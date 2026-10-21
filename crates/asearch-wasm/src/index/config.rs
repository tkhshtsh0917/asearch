use asearch_rs::{
    AmbigPolicy, LengthUnit, MatchMode, Normalizer, Profile, QueryOptions, ResourceLimits,
    SearchError, TransposedOptions,
};

pub(crate) fn index_config(
    config: &[u32],
) -> Result<(TransposedOptions, ResourceLimits), SearchError> {
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

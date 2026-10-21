//! Bounded UTF-16 column import for cross-language fixtures and server adapters.
use serde::Deserialize;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    units: Vec<u16>,
    lengths: Vec<u32>,
    keys: Vec<u32>,
    orders: Vec<u32>,
    tombstones: Vec<u32>,
    config: Vec<u32>,
}
pub fn build(value: serde_json::Value) -> crate::Result<Vec<u8>> {
    let input: Input = serde_json::from_value(value)?;
    let (options, limits) = asearch_index::index_config(&input.config)?;
    Ok(asearch_builder::build_flat(
        &input.units,
        &input.lengths,
        &input.keys,
        &input.orders,
        &input.tombstones,
        options,
        limits,
    )?)
}

use asearch_engine::{
    AsearchIndex, ExpectedSegment, ResourceLimits, SearchError, TransposedOptions,
};

pub(super) fn load(
    bytes: Vec<u8>,
    options: TransposedOptions,
    limits: ResourceLimits,
    wire: &[u8],
) -> Result<AsearchIndex, SearchError> {
    match decode(wire)? {
        None => AsearchIndex::from_owned_bytes(bytes, options, limits),
        Some(expected) => {
            AsearchIndex::from_owned_bytes_expected(bytes, options, limits, &expected)
        }
    }
}
pub(super) fn decode(wire: &[u8]) -> Result<Option<ExpectedSegment>, SearchError> {
    if wire.is_empty() {
        return Ok(None);
    }
    if wire.len() != 64 {
        return Err(SearchError::InvalidInput(
            "invalid expected segment descriptor",
        ));
    }
    let expected = ExpectedSegment {
        id: u64::from_le_bytes(std::array::from_fn(|i| wire[i])),
        bytes: u32::from_le_bytes(std::array::from_fn(|i| wire[8 + i])) as usize,
        texts: u32::from_le_bytes(std::array::from_fn(|i| wire[12 + i])) as usize,
        sha384: std::array::from_fn(|i| wire[16 + i]),
    };
    Ok(Some(expected))
}

use super::{checksum, invalid, u16_at, u32_at, u64_at};
use crate::{
    Normalizer, Profile, ResourceLimits, SearchError, TransposedOptions,
    capacity::{Budget, add, mul},
};

#[derive(Clone, Copy)]
struct Section {
    tag: u32,
    flags: u32,
    start: usize,
    end: usize,
}

pub(super) struct View<'a> {
    pub texts: usize,
    pub tombstones: Option<&'a [u8]>,
    pub normalized: &'a [u8],
    pub lengths: &'a [u8],
    pub keys: &'a [u8],
    pub orders: Option<&'a [u8]>,
    pub original: Option<&'a [u8]>,
    pub original_lengths: Option<&'a [u8]>,
}

impl<'a> View<'a> {
    pub fn parse(
        bytes: &'a [u8],
        options: TransposedOptions,
        limits: ResourceLimits,
        reserved_input: usize,
    ) -> Result<Self, SearchError> {
        let (count, header, texts, units) = validate_header(bytes, options, limits)?;
        let sections = read_sections(bytes, count, header, limits, reserved_input)?;
        let mut view = Self {
            texts,
            tombstones: None,
            normalized: &[],
            lengths: &[],
            keys: &[],
            orders: None,
            original: None,
            original_lengths: None,
        };
        let mut required = 0;
        for section in &sections {
            let data = &bytes[section.start..section.end];
            let tag = section.tag.to_le_bytes();
            match &tag {
                b"TEXT" if section.flags == 1 && data.len() == mul(units, 2)? => {
                    view.normalized = data;
                    required |= 1;
                }
                b"TLEN" if section.flags == 1 && data.len() == mul(texts, 2)? => {
                    view.lengths = data;
                    required |= 2;
                }
                b"KEYS" if section.flags == 1 && data.len() == mul(texts, 4)? => {
                    view.keys = data;
                    required |= 4;
                }
                b"ORDR" if section.flags == 1 && data.len() == mul(texts, 4)? => {
                    view.orders = Some(data);
                }
                b"TOMB"
                    if section.flags == 1 && data.len() == mul(u32_at(bytes, 44) as usize, 4)? =>
                {
                    view.tombstones = Some(data);
                }
                b"OLEN" if section.flags == 0 && data.len() == mul(texts, 2)? => {
                    view.original_lengths = Some(data);
                }
                b"OTXT" if section.flags == 0 && data.len().is_multiple_of(2) => {
                    view.original = Some(data);
                }
                b"TEXT" | b"TLEN" | b"KEYS" | b"ORDR" | b"TOMB" | b"OLEN" | b"OTXT" => {
                    return Err(invalid());
                }
                _ if section.flags == 1 => return Err(invalid()),
                _ => {}
            }
        }
        if required != 7 || view.original.is_some() != view.original_lengths.is_some() {
            return Err(invalid());
        }
        if u32_at(bytes, 44) != 0 && view.tombstones.is_none() {
            return Err(invalid());
        }
        if let Some(tombs) = view.tombstones {
            for i in 1..tombs.len() / 4 {
                if u32_at(tombs, (i - 1) * 4) >= u32_at(tombs, i * 4) {
                    return Err(invalid());
                }
            }
        }
        view.validate_rows()?;
        Ok(view)
    }

    pub fn key(&self, i: usize) -> u32 {
        u32_at(self.keys, i * 4)
    }
    pub fn order(&self, i: usize) -> u32 {
        self.orders
            .map_or_else(|| self.key(i), |bytes| u32_at(bytes, i * 4))
    }

    fn validate_rows(&self) -> Result<(), SearchError> {
        let mut units = 0;
        let mut original_units = 0;
        for i in 0..self.texts {
            units = add(units, u16_at(self.lengths, i * 2) as usize)?;
            if let Some(lengths) = self.original_lengths {
                original_units = add(original_units, u16_at(lengths, i * 2) as usize)?;
            }
            if i > 0
                && ((self.order(i), self.key(i)) < (self.order(i - 1), self.key(i - 1))
                    || self.key(i) == self.key(i - 1) && self.order(i) != self.order(i - 1))
            {
                return Err(invalid());
            }
        }
        let original_bytes = mul(original_units, 2)?;
        if mul(units, 2)? != self.normalized.len()
            || self
                .original
                .is_some_and(|bytes| bytes.len() != original_bytes)
        {
            return Err(invalid());
        }
        Ok(())
    }
}

fn validate_header(
    bytes: &[u8],
    options: TransposedOptions,
    limits: ResourceLimits,
) -> Result<(usize, usize, usize, usize), SearchError> {
    limits.validate()?;
    options.validate()?;
    if bytes.len() > limits.max_total_bytes {
        return Err(SearchError::ResourceLimit("ASIX input byte limit"));
    }
    if bytes.len() < 64
        || &bytes[..4] != b"ASIX"
        || u16_at(bytes, 4) != 1
        || u64_at(bytes, 16) != bytes.len() as u64
        || bytes[33..36] != [0; 3]
        || bytes[56..64] != [0; 8]
    {
        return Err(invalid());
    }
    let normalizer =
        Normalizer::from_descriptor(u16_at(bytes, 48), u16_at(bytes, 50), u32_at(bytes, 52))
            .map_err(|_| invalid())?;
    if normalizer != options.normalizer
        || bytes[32] != u8::from(options.profile == Profile::AsearchTsLib)
    {
        return Err(invalid());
    }
    let count = u32_at(bytes, 12) as usize;
    let header = add(64, mul(count, 16)?)?;
    if header != u32_at(bytes, 8) as usize
        || header > bytes.len()
        || checksum(bytes) != u64_at(bytes, 24)
    {
        return Err(invalid());
    }
    let texts = u32_at(bytes, 36) as usize;
    let units = u32_at(bytes, 40) as usize;
    if texts > limits.max_texts || units > limits.max_units {
        return Err(SearchError::ResourceLimit("ASIX count limit"));
    }
    Ok((count, header, texts, units))
}

fn read_sections(
    bytes: &[u8],
    count: usize,
    header: usize,
    limits: ResourceLimits,
    reserved_input: usize,
) -> Result<Vec<Section>, SearchError> {
    let mut budget = Budget::new(reserved_input, limits.max_working_bytes)?;
    let mut sections = budget.reserve(count)?;
    for i in 0..count {
        let offset = 64 + i * 16;
        let start = u32_at(bytes, offset + 8) as usize;
        let end = add(start, u32_at(bytes, offset + 12) as usize)?;
        let flags = u32_at(bytes, offset + 4);
        if start < header || !start.is_multiple_of(16) || end > bytes.len() || flags & !1 != 0 {
            return Err(invalid());
        }
        sections.push(Section {
            tag: u32_at(bytes, offset),
            flags,
            start,
            end,
        });
    }
    sections.sort_unstable_by_key(|s| s.tag);
    if sections.windows(2).any(|pair| pair[0].tag == pair[1].tag) {
        return Err(invalid());
    }
    sections.sort_unstable_by_key(|s| (s.start, s.end));
    let mut end = header;
    for section in &sections {
        if section.start < end || bytes[end..section.start].iter().any(|&b| b != 0) {
            return Err(invalid());
        }
        end = section.end;
    }
    if bytes[end..].iter().any(|&b| b != 0) {
        return Err(invalid());
    }
    Ok(sections)
}

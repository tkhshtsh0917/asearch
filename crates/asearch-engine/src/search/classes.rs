//! Literal classes shared by bounded scalar display and DAG matching.
use crate::{MatchMode, QueryWorkspace, SearchError, capacity::Budget};
#[derive(Clone, Copy)]
pub(crate) struct Class {
    pub keys: [u32; 3],
    pub looping: bool,
}

pub(crate) fn compile(
    query: &QueryWorkspace,
    casing: Option<&[u32]>,
    budget: &mut Budget,
) -> Result<(Vec<Class>, bool), SearchError> {
    if let Some(keys) = casing {
        crate::transposed::casing::validate(query, keys)?;
    }
    let mut classes: Vec<Class> = budget.reserve(query.pattern().len())?;
    let mut leading = query.options().mode == MatchMode::Include;
    for (i, value) in std::char::decode_utf16(query.pattern().iter().copied()).enumerate() {
        let value = value.map_or_else(|e| u32::from(e.unpaired_surrogate()), u32::from);
        if query.options().mode == MatchMode::WildcardSpace && value == 32 {
            if let Some(last) = classes.last_mut() {
                last.looping = true;
            } else {
                leading = true;
            }
            continue;
        }
        let mut keys = [value, u32::MAX, u32::MAX];
        if let Some(casing) = casing {
            keys.copy_from_slice(&casing[i * 3..i * 3 + 3]);
        } else if let Some(c) = char::from_u32(value) {
            fn single(mut chars: impl Iterator<Item = char>) -> u32 {
                let first = chars.next();
                if chars.next().is_none() {
                    first.map_or(u32::MAX, u32::from)
                } else {
                    u32::MAX
                }
            }
            keys[1] = single(c.to_lowercase());
            keys[2] = single(c.to_uppercase());
        }
        classes.push(Class {
            keys,
            looping: false,
        });
    }
    if query.options().mode == MatchMode::Include
        && let Some(last) = classes.last_mut()
    {
        last.looping = true;
    }
    Ok((classes, leading))
}

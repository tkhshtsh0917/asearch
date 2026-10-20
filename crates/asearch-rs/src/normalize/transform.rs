use super::tables::{COMPOSITIONS, HALFWIDTH};

fn width(unit: u16, options: u32) -> u16 {
    match unit {
        0x3000 if options & 1 != 0 => 0x20,
        0xff01..=0xff5e if options & 2 != 0 => unit - 0xfee0,
        0xff61..=0xff9f if options & 4 != 0 => HALFWIDTH[usize::from(unit - 0xff61)],
        _ => unit,
    }
}

fn compose(base: u16, mark: u16) -> Option<u16> {
    COMPOSITIONS
        .binary_search_by_key(&(base, mark), |&(b, m, _)| (b, m))
        .ok()
        .map(|i| COMPOSITIONS[i].2)
}

fn fold(unit: u16, options: u32) -> u16 {
    let unit = match unit {
        0x30a1..=0x30f6 if options & 8 != 0 => unit - 0x60,
        _ => unit,
    };
    let unit = match unit {
        0x41..=0x5a if options & 16 != 0 => unit + 0x20,
        _ => unit,
    };
    match unit {
        0x002d | 0x2010 | 0x2015 | 0x2212 | 0x30fc | 0xff0d | 0xff70 if options & 32 != 0 => 0x30fc,
        _ => unit,
    }
}

pub(super) fn normalize(input: &[u16], output: &mut Vec<u16>, options: u32) {
    mapped(input, options, |unit, _, _| output.push(unit));
}

pub(super) fn mapped(input: &[u16], options: u32, mut emit: impl FnMut(u16, usize, usize)) {
    let mut i = 0;
    while i < input.len() {
        let start = i;
        let mut unit = width(input[i], options);
        // Compose before hiragana folding: ワ + ﾞ becomes ヷ, which lies
        // outside the deliberately limited katakana-to-hiragana range.
        if options & 4 != 0
            && let Some(&mark @ (0xff9e | 0xff9f)) = input.get(i + 1)
            && let Some(combined) = compose(unit, width(mark, options))
        {
            unit = combined;
            i += 1;
        }
        emit(fold(unit, options), start, i + 1);
        i += 1;
    }
}

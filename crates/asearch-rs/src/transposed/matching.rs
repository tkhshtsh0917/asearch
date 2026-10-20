use super::{
    Block, TransposedEngine, symbols,
    workspace::{CACHE_ROWS, Class, TransposedWorkspace},
};
use crate::{MatchMode, QueryWorkspace};

pub(super) fn search(
    engine: &TransposedEngine,
    query: &QueryWorkspace,
    ws: &mut TransposedWorkspace,
) {
    let leading_loop = compile(engine, query, &mut ws.classes);
    ws.levels.fill(None);
    ws.stats.blocks = engine.blocks.len();
    ws.stats.early_exit_blocks = 0;
    // The common small K values are monomorphized. Larger K shares one path.
    match query.max_ambig() {
        0 => run::<1>(engine, ws, leading_loop, 1),
        1 => run::<2>(engine, ws, leading_loop, 2),
        2 => run::<3>(engine, ws, leading_loop, 3),
        3 => run::<4>(engine, ws, leading_loop, 4),
        k => run::<0>(engine, ws, leading_loop, k as usize + 1),
    }
}

fn compile(engine: &TransposedEngine, query: &QueryWorkspace, classes: &mut Vec<Class>) -> bool {
    classes.clear();
    let mode = query.options().mode;
    let mut leading_loop = mode == MatchMode::Include;
    for symbol in symbols(query.pattern()) {
        if mode == MatchMode::WildcardSpace && symbol == 0x20 {
            if let Some(last) = classes.last_mut() {
                last.loop_after = true;
            } else {
                leading_loop = true;
            }
            continue;
        }
        let mut ids = [engine.eq.id(symbol), 0, 0];
        if let Some(c) = char::from_u32(symbol) {
            ids[1] = single(c.to_lowercase()).map_or(0, |s| engine.eq.id(s));
            ids[2] = single(c.to_uppercase()).map_or(0, |s| engine.eq.id(s));
        }
        ids.sort_unstable();
        classes.push(Class {
            ids,
            loop_after: false,
        });
    }
    if mode == MatchMode::Include
        && let Some(last) = classes.last_mut()
    {
        last.loop_after = true;
    }
    leading_loop
}

fn single(mut chars: impl Iterator<Item = char>) -> Option<u32> {
    let c = chars.next()?;
    if chars.next().is_none() {
        Some(u32::from(c))
    } else {
        None
    }
}

fn run<const FIXED: usize>(
    engine: &TransposedEngine,
    ws: &mut TransposedWorkspace,
    leading: bool,
    dynamic: usize,
) {
    let levels = if FIXED == 0 { dynamic } else { FIXED };
    for block in &engine.blocks {
        ws.current.fill(0);
        ws.previous.fill(0);
        let mut keys = [[0; 3]; CACHE_ROWS];
        let mut valid = [false; CACHE_ROWS];
        let mut replace = 0;
        row(
            engine,
            *block,
            &ws.previous,
            &mut ws.current,
            &[],
            ws.words,
            levels,
            0,
            leading,
        );
        std::mem::swap(&mut ws.previous, &mut ws.current);
        let mut dead = false;

        for (i, class) in ws.classes.iter().enumerate() {
            let cached = (0..CACHE_ROWS).find(|&c| valid[c] && keys[c] == class.ids);
            let cache = cached.unwrap_or_else(|| {
                let c = replace;
                replace = (replace + 1) % CACHE_ROWS;
                keys[c] = class.ids;
                valid[c] = true;
                let output = &mut ws.eq_cache[c * ws.words..c * ws.words + block.words];
                output.fill(0);
                let mut previous_id = 0;
                for id in class.ids {
                    if id != previous_id {
                        engine.eq.scatter(id, block.start, output);
                    }
                    previous_id = id;
                }
                c
            });
            let eq = &ws.eq_cache[cache * ws.words..cache * ws.words + block.words];
            row(
                engine,
                *block,
                &ws.previous,
                &mut ws.current,
                eq,
                ws.words,
                levels,
                i + 1,
                class.loop_after,
            );
            std::mem::swap(&mut ws.previous, &mut ws.current);
            if i + 1 >= levels
                && ws.previous[(levels - 1) * ws.words..(levels - 1) * ws.words + block.words]
                    .iter()
                    .all(|&v| v == 0)
            {
                dead = true;
                break;
            }
        }
        if !dead {
            extract(
                engine,
                *block,
                &ws.previous,
                ws.words,
                levels,
                &mut ws.levels,
            );
        }
        ws.stats.early_exit_blocks += usize::from(dead);
    }
}

#[allow(clippy::too_many_arguments)]
fn row(
    engine: &TransposedEngine,
    block: Block,
    previous: &[u64],
    current: &mut [u64],
    eq: &[u64],
    stride: usize,
    levels: usize,
    i: usize,
    looping: bool,
) {
    for e in 0..levels {
        let base = e * stride;
        let mut raw_before = 0;
        let mut addition_carry = false;
        for w in 0..block.words {
            let mask = engine.masks[block.start + w];
            let shift = |values: &[u64], start: usize| {
                (values[start + w] << 1)
                    | if w == 0 {
                        0
                    } else {
                        values[start + w - 1] >> 63
                    }
            };
            let mut x = if i == 0 {
                0
            } else {
                shift(previous, base) & eq[w]
            };
            if e > 0 {
                let lower = base - stride;
                x |= shift(current, lower);
                if i > 0 {
                    x |= shift(previous, lower) | previous[lower + w];
                }
            }
            // Clear other texts' boundary columns and every dead slot.
            x &= mask.text;
            if i <= e {
                x |= mask.sep;
            }
            let value = if looping {
                let seeds = ((x & mask.sep) << 1) | (raw_before >> 63);
                let y = (x & mask.text) | (seeds & mask.text);
                let (sum, a) = y.overflowing_add(mask.text);
                let (sum, b) = sum.overflowing_add(u64::from(addition_carry));
                addition_carry = a || b;
                (((sum ^ mask.text) & mask.text) | y) | (x & mask.sep)
            } else {
                x
            };
            raw_before = x & mask.sep;
            current[base + w] = value;
        }
    }
}

fn extract(
    engine: &TransposedEngine,
    block: Block,
    states: &[u64],
    stride: usize,
    levels: usize,
    output: &mut [Option<u16>],
) {
    for w in 0..block.words {
        let mask = engine.masks[block.start + w];
        let mut ends = mask.end;
        let mut text = mask.end_before;
        while ends != 0 {
            let bit = 1_u64 << ends.trailing_zeros();
            for e in 0..levels {
                if states[e * stride + w] & bit != 0 {
                    output[text] = Some(e as u16);
                    break;
                }
            }
            ends &= ends - 1;
            text += 1;
        }
    }
}

//! Extract every document once. Candidate capture visits every matching text,
//! including aliases outside top-N and dead rows, during that same pass.

use super::{
    AsearchIndex, SearchHit, SearchResults, narrowing::Narrowing, workspace::OutputBuffers,
};
pub(super) struct Input<'a> {
    pub index: &'a AsearchIndex,
    pub levels: &'a [Option<u16>],
    pub k: u8,
    pub limit: usize,
    pub count_total: bool,
    pub live: Option<&'a [u8]>,
    pub positional: Option<(
        &'a [crate::transposed::workspace::Class],
        bool,
        crate::Profile,
    )>,
}

pub(super) fn extract<'w>(
    input: Input<'_>,
    ws: &'w mut OutputBuffers,
    planner: Option<&mut Narrowing>,
) -> SearchResults<'w> {
    match planner {
        Some(planner) => run::<true>(input, ws, Some(planner)),
        None => run::<false>(input, ws, None),
    }
}

fn run<'w, const CAPTURE: bool>(
    input: Input<'_>,
    ws: &'w mut OutputBuffers,
    mut planner: Option<&mut Narrowing>,
) -> SearchResults<'w> {
    let Input {
        index,
        levels,
        k,
        limit,
        count_total,
        live,
        positional,
    } = input;
    ws.hits.clear();
    ws.totals.fill(0);
    for (i, (doc, best)) in index.documents.iter().zip(&mut ws.best).enumerate() {
        *best = None;
        let active = live.is_none_or(|mask| mask[i] != 0);
        if !active && !CAPTURE {
            continue;
        }
        for (offset, &level) in levels[doc.start..doc.end].iter().enumerate() {
            if let Some(level) = level {
                let text_id = doc.start + offset;
                if CAPTURE {
                    planner
                        .as_deref_mut()
                        .expect("capture enabled")
                        .capture_candidate(index, text_id);
                }
                // Candidate capture stays complete even when this alias cannot win.
                if positional.is_some()
                    && best.is_some_and(|hit| {
                        level > hit.level || (level == hit.level && hit.start == 0)
                    })
                {
                    continue;
                }
                let start = if active {
                    positional.map_or(usize::MAX, |(classes, leading, profile)| {
                        super::position::start(
                            index,
                            text_id,
                            classes,
                            leading,
                            profile,
                            &mut ws.positions,
                        )
                    })
                } else {
                    usize::MAX
                };
                if active && best.is_none_or(|hit| (level, start) < (hit.level, hit.start)) {
                    *best = Some(SearchHit {
                        key: doc.key,
                        order: doc.order,
                        text_id,
                        level,
                        start,
                    });
                }
            }
        }
        if count_total && let Some(hit) = best {
            ws.totals[hit.level as usize] += 1;
        }
    }
    if positional.is_some() {
        ws.hits.extend(ws.best.iter().flatten().copied());
        ws.hits
            .sort_unstable_by_key(|hit| (hit.level, hit.start, hit.order, hit.key));
        ws.hits.truncate(limit);
        return SearchResults {
            hits: &ws.hits,
            totals: count_total.then_some(&ws.totals[..=k as usize]),
        };
    }
    // Canonical document order and level buckets preserve (level,order,key).
    'levels: for level in 0..=k {
        for &hit in ws.best.iter().flatten() {
            if ws.hits.len() >= limit {
                break 'levels;
            }
            if hit.level == u16::from(level) {
                ws.hits.push(hit);
            }
        }
    }
    SearchResults {
        hits: &ws.hits,
        totals: count_total.then_some(&ws.totals[..=k as usize]),
    }
}

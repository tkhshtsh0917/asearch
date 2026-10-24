use asearch_engine::{
    Asearch, AsearchIndex, Doc, Engine, IndexSet, MatchMode, MultiWord, Normalizer, Profile,
    QueryOptions, QueryWorkspace, ResourceLimits, ScalarEngine, ScalarLimits, ScalarWm,
    TransposedEngine, TransposedOptions,
};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

struct Counter;
static ENABLED: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
// SAFETY: Every allocation/deallocation is forwarded unchanged to System;
// atomics only observe calls during the single test's measured section.
unsafe impl GlobalAlloc for Counter {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if ENABLED.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: The caller's valid layout is forwarded without modification.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: ptr and layout are returned to the same backing allocator.
        unsafe { System.dealloc(ptr, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: Counter = Counter;

#[test]
fn warm_matching_does_not_allocate() {
    for profile in [Profile::AsearchTs, Profile::AsearchTsLib, Profile::Ideal] {
        let a = Asearch::with_profile("漢 字😀", MatchMode::WildcardSpace, profile).unwrap();
        let units: Vec<_> = "漢の字😀".encode_utf16().collect();
        ENABLED.store(true, Ordering::SeqCst);
        for e in 0..=31 {
            std::hint::black_box(a.is_match("漢の字😀", e));
            std::hint::black_box(a.is_match_utf16(&units, e));
            let _ = std::hint::black_box(a.match_level("漢の字😀", e)).unwrap();
            let _ = std::hint::black_box(a.match_level_utf16(&units, e)).unwrap();
        }
        ENABLED.store(false, Ordering::SeqCst);
    }
    let pattern = ScalarWm::<MultiWord<3>>::new(
        &"漢 字😀".repeat(20),
        MatchMode::WildcardSpace,
        ScalarLimits::default(),
    )
    .unwrap();
    let text = "漢の字😀".repeat(20);
    let mut workspace = pattern.workspace(31).unwrap();
    ENABLED.store(true, Ordering::SeqCst);
    for _ in 0..100 {
        let _ = std::hint::black_box(pattern.match_level(&text, &mut workspace)).unwrap();
    }
    ENABLED.store(false, Ordering::SeqCst);
    let texts = [text.as_str(), "", "漢字"];
    let engine = ScalarEngine::new(&texts, ScalarLimits::default()).unwrap();
    let mut workspace = engine.workspace(&pattern, 3).unwrap();
    ENABLED.store(true, Ordering::SeqCst);
    for _ in 0..100 {
        let _ = std::hint::black_box(engine.search(&pattern, &mut workspace)).unwrap();
    }
    ENABLED.store(false, Ordering::SeqCst);
    let input: Vec<_> = "　ＡＢＣｶﾞﾊﾟｳﾞ😀".encode_utf16().collect();
    let mut output = Vec::with_capacity(input.len());
    ENABLED.store(true, Ordering::SeqCst);
    for _ in 0..100 {
        Normalizer::JA_BASIC
            .normalize_into(&input, &mut output)
            .unwrap();
    }
    ENABLED.store(false, Ordering::SeqCst);

    let mut query =
        QueryWorkspace::new(QueryOptions::default(), ResourceLimits::default()).unwrap();
    ENABLED.store(true, Ordering::SeqCst);
    for _ in 0..100 {
        query.update(&input).unwrap();
        query
            .update_with_options(
                &input,
                QueryOptions {
                    mode: MatchMode::Exact,
                    ..QueryOptions::default()
                },
            )
            .unwrap();
        query
            .update_with_options(&input, QueryOptions::default())
            .unwrap();
        std::hint::black_box(query.pattern());
        std::hint::black_box(query.max_ambig());
    }
    ENABLED.store(false, Ordering::SeqCst);
    let texts = [input.as_slice()];
    let engine = TransposedEngine::new(
        &texts,
        TransposedOptions::default(),
        ResourceLimits::default(),
    )
    .unwrap();

    let mut ws = engine.workspace(&query, 7).unwrap();
    ENABLED.store(true, Ordering::SeqCst);
    for _ in 0..100 {
        let _ = std::hint::black_box(engine.search(&query, &mut ws)).unwrap();
    }
    ENABLED.store(false, Ordering::SeqCst);
    let index = AsearchIndex::from_docs(
        &[Doc {
            key: 1,
            order: 1,
            texts: &texts,
        }],
        TransposedOptions::default(),
        ResourceLimits::default(),
    )
    .unwrap();
    let mut workspace = index.workspace(&query, 7).unwrap();
    workspace
        .enable_narrowing(&index, &query, 4 * 1024 * 1024)
        .unwrap();
    workspace
        .enable_incremental(&index, &query, 4 * 1024 * 1024)
        .unwrap();
    let casing: Vec<_> = std::char::decode_utf16(query.pattern().iter().copied())
        .flat_map(|c| [u32::from(c.unwrap()); 3])
        .collect();
    ENABLED.store(true, Ordering::SeqCst);
    for _ in 0..100 {
        let _ = std::hint::black_box(index.search(&query, &mut workspace, 30, true)).unwrap();
        let _ = std::hint::black_box(index.search_with_casing(
            &query,
            &casing,
            &mut workspace,
            30,
            true,
        ))
        .unwrap();
    }
    ENABLED.store(false, Ordering::SeqCst);
    drop(workspace);
    let set = IndexSet::new(vec![index], ResourceLimits::default()).unwrap();
    let mut workspace = set.workspace(&query, 7).unwrap();
    ENABLED.store(true, Ordering::SeqCst);
    for _ in 0..100 {
        let _ = std::hint::black_box(set.search(&query, &mut workspace, 30, true)).unwrap();
        let _ =
            std::hint::black_box(set.search_with_casing(&query, &casing, &mut workspace, 30, true))
                .unwrap();
    }
    ENABLED.store(false, Ordering::SeqCst);
    warm_sparse_code_values();
    warm_positional_matching();
    assert_eq!(ALLOCS.load(Ordering::SeqCst), 0);
}

// Keeps fixture creation outside the allocation counter and exercises the
// code-value path through the public owning index, including mask compilation.
fn warm_sparse_code_values() {
    let prefix: Vec<_> = "needle anchor\0".encode_utf16().collect();
    let mut target = prefix.clone();
    target.extend("q".repeat(180).encode_utf16());
    let mut data = vec![vec![u16::from(b'z'); 1024]; 128];
    data.push(target);
    let groups: Vec<_> = data.iter().map(|s| [s.as_slice()]).collect();
    let docs: Vec<_> = groups
        .iter()
        .enumerate()
        .map(|(key, texts)| Doc {
            key: key as u32,
            order: key as u32,
            texts,
        })
        .collect();

    for profile in [Profile::AsearchTs, Profile::AsearchTsLib, Profile::Ideal] {
        let index = AsearchIndex::from_docs(
            &docs,
            TransposedOptions {
                profile,
                ..TransposedOptions::default()
            },
            ResourceLimits::default(),
        )
        .unwrap();
        let mut query = QueryWorkspace::new(
            QueryOptions {
                profile,
                ..QueryOptions::default()
            },
            ResourceLimits::default(),
        )
        .unwrap();
        let mut workspace = index.workspace(&query, 7).unwrap();
        workspace
            .enable_narrowing(&index, &query, usize::MAX)
            .unwrap();
        query.update(&prefix[..13]).unwrap();
        index.search(&query, &mut workspace, 30, true).unwrap();
        query.update(&prefix).unwrap();

        ENABLED.store(true, Ordering::SeqCst);
        for _ in 0..100 {
            let _ = std::hint::black_box(index.search(&query, &mut workspace, 30, true)).unwrap();
        }
        ENABLED.store(false, Ordering::SeqCst);
        assert_eq!(workspace.stats().narrowed_texts, Some(1));
    }
}

// Positional extraction includes global sorting and planner reuse, all on reserved buffers.
fn warm_positional_matching() {
    let units: Vec<Vec<u16>> = ["possibility", "xbi", "bi", "bi alias"]
        .iter()
        .map(|text| text.encode_utf16().collect())
        .collect();
    let texts: Vec<_> = units.iter().map(Vec::as_slice).collect();
    let index = AsearchIndex::from_docs(
        &[Doc {
            key: 1,
            order: 1,
            texts: &texts,
        }],
        TransposedOptions::default(),
        ResourceLimits::default(),
    )
    .unwrap();
    let set = IndexSet::new(vec![index], ResourceLimits::default()).unwrap();
    let mut query = QueryWorkspace::new(
        QueryOptions {
            rank_by_position: true,
            ..QueryOptions::default()
        },
        ResourceLimits::default(),
    )
    .unwrap();
    let mut ws = set.workspace(&query, 7).unwrap();
    ws.enable_planner(&set, &query, 1024 * 1024, 0).unwrap();
    let patterns: Vec<Vec<u16>> = ["b", "bi", "bix", "bi"]
        .iter()
        .map(|pattern| pattern.encode_utf16().collect())
        .collect();

    ENABLED.store(true, Ordering::SeqCst);
    for pattern in &patterns {
        query.update(pattern).unwrap();
        let _ = std::hint::black_box(set.search(&query, &mut ws, 2, true)).unwrap();
    }
    ENABLED.store(false, Ordering::SeqCst);
}

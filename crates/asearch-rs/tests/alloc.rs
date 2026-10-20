use asearch_rs::{
    Asearch, Engine, MatchMode, MultiWord, Normalizer, Profile, QueryOptions, QueryWorkspace,
    ResourceLimits, ScalarEngine, ScalarLimits, ScalarWm, TransposedEngine, TransposedOptions,
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
    assert_eq!(ALLOCS.load(Ordering::SeqCst), 0);
}

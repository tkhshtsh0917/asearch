//! Checks that real allocations are detected without observing other threads or scopes.

use super::counter::Measurement;
use std::{
    alloc::{Layout, alloc, alloc_zeroed, dealloc, realloc},
    hint::{black_box, spin_loop},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

fn allocate_once() -> *mut u8 {
    // SAFETY: The layout is nonzero and valid; callers release the returned allocation.
    unsafe { black_box(alloc(Layout::from_size_align(64, 8).unwrap())) }
}

fn release(pointer: *mut u8, size: usize) {
    assert!(!pointer.is_null());
    // SAFETY: The pointer is the live System allocation with this size/alignment.
    unsafe { dealloc(pointer, Layout::from_size_align(size, 8).unwrap()) };
}

#[test]
fn detects_alloc_zeroed_and_realloc_calls() {
    let layout = Layout::from_size_align(64, 8).unwrap();
    let scope = Measurement::start();
    let pointer = allocate_once();
    assert_eq!(scope.finish(), 1);
    assert!(!pointer.is_null());

    let scope = Measurement::start();
    // SAFETY: The original allocation is live, with layout and a valid new size.
    let resized = unsafe { black_box(realloc(pointer, layout, 128)) };
    assert_eq!(scope.finish(), 1);
    if resized.is_null() {
        release(pointer, 64);
        panic!("reallocation failed");
    }
    release(resized, 128);

    let scope = Measurement::start();
    // SAFETY: The layout is nonzero and valid.
    let zeroed = unsafe { black_box(alloc_zeroed(layout)) };
    assert_eq!(scope.finish(), 1);
    assert!(!zeroed.is_null());
    // SAFETY: The 64-byte allocation is live and initialized to zero.
    assert!(
        unsafe { std::slice::from_raw_parts(zeroed, 64) }
            .iter()
            .all(|&v| v == 0)
    );
    release(zeroed, 64);
}

#[test]
fn ignores_other_threads_while_both_threads_measure() {
    struct Completion<'a>(&'a AtomicBool);
    impl Drop for Completion<'_> {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }
    let start = AtomicBool::new(false);
    let done = AtomicBool::new(false);
    let other_count = AtomicUsize::new(0);
    std::thread::scope(|threads| {
        let worker = threads.spawn(|| {
            let _completion = Completion(&done);
            while !start.load(Ordering::Acquire) {
                spin_loop();
            }
            let scope = Measurement::start();
            let pointer = allocate_once();
            other_count.store(scope.finish(), Ordering::Relaxed);
            release(pointer, 64);
        });

        let scope = Measurement::start();
        start.store(true, Ordering::Release);
        while !done.load(Ordering::Acquire) {
            spin_loop();
        }
        let count = scope.finish();

        worker.join().unwrap();
        assert_eq!(count, 0);
        assert_eq!(other_count.load(Ordering::Relaxed), 1);
    });
}

#[test]
fn ignores_setup_and_drops_outside_the_scope() {
    let pointer = allocate_once();
    let scope = Measurement::start();
    release(pointer, 64);
    assert_eq!(scope.finish(), 0);
    assert_eq!(Measurement::start().finish(), 0);
}

#[test]
fn unwinding_disables_the_scope() {
    assert!(
        catch_unwind(|| {
            let _scope = Measurement::start();
            panic!("injected measurement failure");
        })
        .is_err()
    );
    assert_eq!(Measurement::start().finish(), 0);
}

#[test]
fn nested_measurement_rejects_without_resetting_its_owner() {
    let scope = Measurement::start();
    assert!(catch_unwind(AssertUnwindSafe(Measurement::start)).is_err());
    let pointer = allocate_once();
    assert!(scope.finish() > 0);
    release(pointer, 64);
    assert_eq!(Measurement::start().finish(), 0);
}

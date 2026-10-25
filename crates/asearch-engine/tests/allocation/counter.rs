//! Counts allocation calls made by the current thread within an unwind-safe scope.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    marker::PhantomData,
    rc::Rc,
};

thread_local! {
    // Const TLS initialization and the counter updates do not allocate.
    static CALLS: Cell<Option<usize>> = const { Cell::new(None) };
}

fn record() {
    // Allocations during TLS teardown must still reach the backing allocator.
    let _ = CALLS.try_with(|calls| {
        if let Some(count) = calls.get() {
            calls.set(Some(count.saturating_add(1)));
        }
    });
}

pub struct CountingAllocator;

// SAFETY: Layouts and pointers are forwarded unchanged to System. Observation
// only updates nonallocating thread-local state and never calls user code.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record();
        // SAFETY: The caller supplies a valid allocation layout.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record();
        // SAFETY: The caller supplies a valid allocation layout.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record();
        // SAFETY: The caller supplies System's live allocation and valid new size.
        unsafe { System.realloc(ptr, layout, size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: The caller returns the allocation to its backing allocator.
        unsafe { System.dealloc(ptr, layout) }
    }
}

/// Observes one synchronous section; the scope cannot move to another thread.
pub struct Measurement {
    _thread: PhantomData<Rc<()>>,
}

impl Measurement {
    pub fn start() -> Self {
        CALLS.with(|calls| {
            assert!(calls.get().is_none(), "nested allocation measurement");
            calls.set(Some(0));
        });
        Self {
            _thread: PhantomData,
        }
    }

    pub fn finish(self) -> usize {
        let count = CALLS.with(|calls| calls.get().expect("active measurement"));
        drop(self);
        count
    }
}

impl Drop for Measurement {
    fn drop(&mut self) {
        let _ = CALLS.try_with(|calls| calls.set(None));
    }
}

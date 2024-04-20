
use std::{
    sync::atomic::{
        AtomicUsize,
        Ordering,
    },
    alloc::{
        Layout,
        alloc,
        dealloc,
        handle_alloc_error,
    },
    fmt::{self, Formatter, Debug},
};

// internals
// ---------
//
// the inner atomic usize is a nullable pointer to a heap allocation.
// the pointed-to data consists of:
//
// - an `unsafe fn(bool, *mut u8)` which, when called with the pointer:
//
//   - if the bool is true, runs the callback (dropping it)
//   - if the bool is false, drops the callback without running it
//   - deallocates the heap allocation
// - padding
// - the `F: FnOnce() + Send + 'static` value

/// Like an `Atomic<Option<Box<dyn FnOnce() + Send + 'static>>>`.
///
/// See [`CallbackCellArgs`][crate::CallbackCellArgs] for a version with args.
pub struct CallbackCell(AtomicUsize);

impl CallbackCell {
    /// Construct with no callback.
    pub fn new() -> Self {
        CallbackCell(AtomicUsize::new(0))
    }

    /// Atomically set the callback.
    pub fn put<F: FnOnce() + Send + 'static>(&self, f: F) {
        unsafe {
            // allocate and initialize heap allocation
            let (layout, callback_offset) = Layout::new::<unsafe fn(bool, *mut u8)>()
                .extend(Layout::new::<F>()).unwrap();
            let ptr = alloc(layout);
            if ptr.is_null() {
                handle_alloc_error(layout);
            }
            (ptr as *mut unsafe fn(bool, *mut u8)).write(fn_ptr_impl::<F>);
            (ptr.add(callback_offset) as *mut F).write(f);

            // atomic put
            let old_ptr = self.0.swap(ptr as usize, Ordering::Release);

            // clean up previous value
            drop_raw(old_ptr as *mut u8);
        }
    }

    /// Atomically take the callback then run it.
    ///
    /// Returns true if a callback was present.
    pub fn take_call(&self) -> bool {
        unsafe {
            // atomic take
            let ptr = self.0.swap(0, Ordering::Acquire) as *mut u8;

            // run it
            if !ptr.is_null() {
                let fn_ptr = (ptr as *mut unsafe fn(bool, *mut u8)).read();
                fn_ptr(true, ptr);
                true
            } else {
                false
            }
        }
    }
}

impl Drop for CallbackCell {
    fn drop(&mut self) {
        unsafe {
            drop_raw(*self.0.get_mut() as *mut u8);
        }
    }
}

// implementation for the function pointer for a given callback type F.
unsafe fn fn_ptr_impl<F: FnOnce() + Send + 'static>(run: bool, ptr: *mut u8) {
    // extract callback value from heap allocation and free heap allocation
    let (layout, callback_offset) = Layout::new::<unsafe fn(bool, *mut u8)>()
        .extend(Layout::new::<F>()).unwrap();
    let f = (ptr.add(callback_offset) as *mut F).read();
    dealloc(ptr, layout);

    // this part is basically safe code
    if run {
        f();
    }
}

// drop the pointed to data, including freeing the heap allocation, without running the callback,
// if the pointer is non-null.
unsafe fn drop_raw(ptr: *mut u8) {
    if !ptr.is_null() {
        let fn_ptr = (ptr as *mut unsafe fn(bool, *mut u8)).read();
        fn_ptr(false, ptr);
    }
}

impl Default for CallbackCell {
    fn default() -> Self {
        Self::new()
    }
}

impl Debug for CallbackCell {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        if (self.0.load(Ordering::Relaxed) as *const ()).is_null() {
            f.write_str("CallbackCell(NULL)")
        } else {
            f.write_str("CallbackCell(NOT NULL)")
        }
    }
}



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
    marker::PhantomData,
    mem::ManuallyDrop,
    fmt::{self, Formatter, Debug},
};

// internals
// ---------
//
// the inner atomic usize is a nullable pointer to a heap allocation.
// the pointed-to data consists of:
//
// - an `unsafe fn(Option<&mut union { I, O }, *mut u8)` which, when
//   called with the pointer:
//
//   - if the option is Some, reads the input from the union, runs the
//     callback with the input (dropping it and the input), and writes
//     the output back to the union
//   - if the option is None, drops the callback without running it
//   - deallocates the heap allocation
// - padding
// - the `F: FnOnce() + Send + 'static` value

/// Like an `Atomic<Option<Box<dyn FnOnce(I) -> O + Send + 'static>>>`.
///
/// It's a normal [`CallbackCell`][crate::CallbackCell] but with args.
pub struct CallbackCellArgs<I, O> {
    ptr: AtomicUsize,
    _p: PhantomData<dyn FnOnce(I) -> O + Send + 'static>,
}

impl<I, O> CallbackCellArgs<I, O> {
    /// Construct with no callback.
    pub fn new() -> Self {
        CallbackCellArgs {
            ptr: AtomicUsize::new(0),
            _p: PhantomData,
        }
    }

    /// Atomically set the callback.
    ///
    /// Makes only one heap allocation. Any callback previously present is dropped.
    pub fn put<F: FnOnce(I) -> O + Send + 'static>(&self, f: F) {
        unsafe {
            // allocate and initialize heap allocation
            let (layout, callback_offset) = Layout::new::<FnPtrType<I, O>>()
                .extend(Layout::new::<F>()).unwrap();
            let ptr = alloc(layout);
            if ptr.is_null() {
                handle_alloc_error(layout);
            }
            (ptr as *mut FnPtrType<I, O>).write(fn_ptr_impl::<I, O, F>);
            (ptr.add(callback_offset) as *mut F).write(f);

            // atomic put
            let old_ptr = self.ptr.swap(ptr as usize, Ordering::Release);

            // clean up previous value
            drop_raw::<I, O>(old_ptr as *mut u8);
        }
    }

    /// Atomically take the callback then run it with the given input.
    ///
    /// Returns the output if a callback was present. If a callback was not
    /// present, returns the original input.
    pub fn take_call(&self, input: I) -> Result<O, I> {
        unsafe {
            // atomic take
            let ptr = self.ptr.swap(0, Ordering::Acquire) as *mut u8;

            // run it
            if !ptr.is_null() {
                let fn_ptr = (ptr as *mut FnPtrType<I, O>).read();
                let mut io_slot = IoSlot { input: ManuallyDrop::new(input) };
                fn_ptr(Some(&mut io_slot), ptr);
                Ok(ManuallyDrop::into_inner(io_slot.output))
            } else {
                Err(input)
            }
        }
    }
}

impl<I, O> Drop for CallbackCellArgs<I, O> {
    fn drop(&mut self) {
        unsafe {
            drop_raw::<I, O>(*self.ptr.get_mut() as *mut u8);
        }
    }
}

union IoSlot<I, O> {
    input: ManuallyDrop<I>,
    output: ManuallyDrop<O>,
}

type FnPtrType<I, O> = unsafe fn(Option<&mut IoSlot<I, O>>, *mut u8);

// implementation for the function pointer for a given callback type F.
unsafe fn fn_ptr_impl<I, O, F>(run: Option<&mut IoSlot<I, O>>, ptr: *mut u8)
where
    F: FnOnce(I) -> O + Send + 'static,
{
    // extract callback value from heap allocation and free heap allocation
    let (layout, callback_offset) = Layout::new::<FnPtrType<I, O>>()
        .extend(Layout::new::<F>()).unwrap();
    let f = (ptr.add(callback_offset) as *mut F).read();
    dealloc(ptr, layout);

    // run
    if let Some(io_slot) = run {
        io_slot.output = ManuallyDrop::new(f(ManuallyDrop::take(&mut io_slot.input)));
    }
}

// drop the pointed to data, including freeing the heap allocation, without running the callback,
// if the pointer is non-null.
unsafe fn drop_raw<I, O>(ptr: *mut u8) {
    if !ptr.is_null() {
        let fn_ptr = (ptr as *mut FnPtrType<I, O>).read();
        fn_ptr(None, ptr);
    }
}

impl<I, O> Default for CallbackCellArgs<I, O> {
    fn default() -> Self {
        Self::new()
    }
}

impl<I, O> Debug for CallbackCellArgs<I, O> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        if (self.ptr.load(Ordering::Relaxed) as *const ()).is_null() {
            f.write_str("CallbackCellArgs(NULL)")
        } else {
            f.write_str("CallbackCellArgs(NOT NULL)")
        }
    }
}

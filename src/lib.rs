//! Like an `Atomic<Option<Box<dyn FnOnce + Send + 'static>>>`.
//!
//! This is a barebones concurrency utility that is useful for building larger
//! abstractions on top of.
//!
//! A naive way of implementing this would involve two layers of indirection:
//! first, the `FnOnce` could be boxed into a `Box<dyn FnOnce>`, achieving
//! dynamic dispatch, and then that could be boxed into a
//! `Box<Box<dyn FnOnce>>`, making it a normal pointer rather than a fat
//! pointer, and then the outer `Box` could be converted into a raw pointer and
//! then into a `usize` and stored in an `AtomicUsize`.
//!
//! This utility, however, does this in only one heap allocation rather than
//! two, through slightly clever usage of monomorphization and the `std::alloc`
//! API.

mod without_args;
mod with_args;

pub use self::{
	without_args::CallbackCell,
	with_args::CallbackCellArgs,
};

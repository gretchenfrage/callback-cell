#![doc = include_str!("../README.md")]

mod without_args;
mod with_args;

pub use self::{
    without_args::CallbackCell,
    with_args::CallbackCellArgs,
};

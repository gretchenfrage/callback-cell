#![doc = include_str!("../../README.md")]

#[cfg(test)]
mod test;

mod without_args;
mod with_args;

pub use self::{
    without_args::CallbackCell,
    with_args::CallbackCellArgs,
};

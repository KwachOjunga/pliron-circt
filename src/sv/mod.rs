//! SystemVerilog emission dialect (`sv`) for [pliron].
//!
//! The dialect preserves target-facing SystemVerilog intent after structural,
//! combinational, and sequential analyses have made the relevant contracts
//! explicit.

pub mod canonicalization;
pub mod lowering;
pub(crate) mod ops;
pub mod parser;
pub mod printer;

pub use ops::*;
use pliron::context::Context;

/// Register the `sv` dialect and its operations in [Context].
pub fn register(ctx: &mut Context) {
    ops::register(ctx);
}

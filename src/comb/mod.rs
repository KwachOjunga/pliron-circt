//! Combinational logic dialect (`comb`) for [pliron].
//!
//! Provides zero-latency arithmetic, logical, comparison, and bit-level operations.

mod canonicalization;
mod cycles;
mod eval;
pub(crate) mod ops;
pub(crate) mod types;
mod utils;

// Re-exports
pub use ops::*;
pub use types::*;

use pliron::context::Context;

/// Register the `comb` dialect and its ops in [Context].
pub fn register(ctx: &mut Context) {
    ops::register(ctx);
}

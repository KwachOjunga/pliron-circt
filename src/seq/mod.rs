//! Sequential hardware dialect (`seq`) for [pliron].
//!
//! The dialect introduces explicit clock-domain values and state boundaries.
//! Combinational behavior remains in `comb`; `seq` records where values are
//! sampled or where clock identity is transformed.

pub(crate) mod ops;
pub(crate) mod types;

pub use ops::*;
use pliron::context::Context;
pub use types::*;

/// Register the `seq` dialect, its types, and its operations in [Context].
pub fn register(ctx: &mut Context) {
    types::register(ctx);
    ops::register(ctx);
}

//! Structural hardware dialect (`hw`) for [pliron].
//!
//! Models modules, hierarchy, ports, and hardware types.

pub(crate) mod ops;
pub(crate) mod types;
pub mod validation;

use pliron::context::Context;

// Re-exports
pub use ops::*;
pub use types::*;

#[doc(inline)]
pub use validation::*;

/// Register the `hw` dialect, its types, and its ops in [Context].
pub fn register(ctx: &mut Context) {
    crate::hw::types::register(ctx);
    crate::hw::ops::register(ctx);
}

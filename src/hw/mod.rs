//! Structural hardware dialect (`hw`) for [pliron].
//!
//! Models modules, hierarchy, ports, and hardware types.

pub mod ops;
pub mod types;
pub mod validation;

use pliron::context::Context;

// Re-exports
pub use ops::*;
pub use types::*;
pub use validation::*;

/// Register the `hw` dialect, its types, and its ops in [Context].
pub fn register(ctx: &mut Context) {
    types::register(ctx);
    ops::register(ctx);
}

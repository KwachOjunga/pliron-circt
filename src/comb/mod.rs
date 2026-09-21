// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Combinational logic dialect (`comb`) for [pliron].
//!
//! Provides zero-latency arithmetic, logical, comparison, and bit-level operations.

mod cycles;
mod eval;
pub mod ops;
pub mod types;

use pliron::context::Context;

/// Register the `comb` dialect and its ops in [Context].
pub fn register(ctx: &mut Context) {
    ops::register(ctx);
}

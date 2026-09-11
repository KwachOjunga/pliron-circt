// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Sequential hardware dialect (`seq`) for [pliron].
//!
//! The dialect introduces explicit clock-domain values and state boundaries.
//! Combinational behavior remains in `comb`; `seq` records where values are
//! sampled or where clock identity is transformed.

pub mod ops;
pub mod types;

use pliron::context::Context;

/// Register the `seq` dialect, its types, and its operations in [Context].
pub fn register(ctx: &mut Context) {
    types::register(ctx);
    ops::register(ctx);
}
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! SystemVerilog emission dialect (`sv`) for [pliron].
//!
//! The dialect preserves target-facing SystemVerilog intent after structural,
//! combinational, and sequential analyses have made the relevant contracts
//! explicit.

pub mod ops;

use pliron::context::Context;

/// Register the `sv` dialect and its operations in [Context].
pub fn register(ctx: &mut Context) {
    ops::register(ctx);
}

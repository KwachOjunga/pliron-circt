// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Hardware dialects for [pliron].

pub mod hw;

use pliron::context::Context;

/// Register all hardware dialects in the given [Context].
pub fn register_all(ctx: &mut Context) {
    hw::register(ctx);
}

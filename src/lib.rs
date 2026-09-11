// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Hardware dialects for [pliron].

pub mod comb;
pub mod hw;
pub mod seq;
pub mod sv;

use pliron::context::Context;

/// Register all hardware dialects in the given [Context].
pub fn register_all(ctx: &mut Context) {
    hw::register(ctx);
    comb::register(ctx);
    seq::register(ctx);
    sv::register(ctx);
}

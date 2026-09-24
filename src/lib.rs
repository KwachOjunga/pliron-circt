// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Hardware dialects for [pliron].

#[cfg(feature = "comb")]
pub mod comb;
#[cfg(feature = "fsm")]
pub mod fsm;
#[cfg(feature = "hw")]
pub mod hw;
#[cfg(feature = "seq")]
pub mod seq;
#[cfg(feature = "sv")]
pub mod sv;

use pliron::context::Context;

pub use pliron;

#[cfg(all(feature = "comb", not(feature = "default")))]
pub fn register_comb(ctx: &mut Context) {
    comb::register(ctx);
}

#[cfg(all(feature = "fsm", not(feature = "default")))]
pub fn register_fsm(ctx: &mut Context) {
    fsm::register(ctx);
}

#[cfg(all(feature = "hw", not(feature = "default")))]
pub fn register_hw(ctx: &mut Context) {
    hw::register(ctx);
}

#[cfg(all(feature = "seq", not(feature = "default")))]
pub fn register_seq(ctx: &mut Context) {
    seq::register(ctx);
}

#[cfg(all(feature = "sv", not(feature = "default")))]
pub fn register_sv(ctx: &mut Context) {
    sv::register(ctx);
}

#[cfg(feature = "default")]
/// Register all hardware dialects in the given [Context].
pub fn register_all(ctx: &mut Context) {
    hw::register(ctx);
    comb::register(ctx);
    seq::register(ctx);
    sv::register(ctx);
    fsm::register(ctx);
}

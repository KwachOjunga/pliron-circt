// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Finite State Machine dialect (`fsm`) for [pliron].
//!
//! Provides target-agnostic finite state machine representations
//! with states, transitions, variables, guards, actions, and hardware/software instantiations.
//! Follows the CIRCT FSM dialect specification and AGENTS.md hardware dialect guidelines.

pub mod canonicalization;
pub mod lowering;
pub mod ops;
pub mod types;

use pliron::context::Context;

/// Register the `fsm` dialect, its types, and its operations in [Context].
pub fn register(ctx: &mut Context) {
    types::register(ctx);
    ops::register(ctx);
}

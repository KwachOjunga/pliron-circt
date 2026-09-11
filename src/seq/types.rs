// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Clock and reset types for the `seq` dialect.

use pliron::{
    context::Context,
    derive::pliron_type,
    r#type::Type,
};

/// A clock waveform with an associated edge and clock-domain identity.
///
/// This is intentionally distinct from `builtin.integer<i1>`: a clock is a
/// temporal resource, not an ordinary Boolean data value.
#[pliron_type(name = "seq.clock", format, verifier = "succ", generate_get = true)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClockType;

/// A reset signal interpreted by a sequential operation.
///
/// Reset polarity and synchronous/asynchronous behavior belong to the
/// consuming operation, so the type does not encode a particular policy.
#[pliron_type(name = "seq.reset", format, verifier = "succ", generate_get = true)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResetType;

/// Register all `seq` types in [Context].
pub fn register(ctx: &mut Context) {
    ClockType::register(ctx);
    ResetType::register(ctx);
}
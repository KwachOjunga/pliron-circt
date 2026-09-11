// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Clock and reset types for the `seq` dialect.

use pliron::{
    context::Context,
    derive::pliron_type,
    r#type::{Type, TypeHandle},
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

/// A synchronous memory resource with a fixed depth and element type.
///
/// The type preserves the storage abstraction until a target-specific memory
/// lowering decides whether it maps to registers, SRAM, or a vendor macro.
#[pliron_type(
    name = "seq.mem",
    format = "`<` $depth `x` $element_type `>`",
    verifier = "succ",
    generate_get = true
)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MemoryType {
    depth: u64,
    element_type: TypeHandle,
}

impl MemoryType {
    /// Number of addressable elements in the memory.
    pub fn depth(&self) -> u64 {
        self.depth
    }

    /// Type stored at every address.
    pub fn element_type(&self) -> TypeHandle {
        self.element_type
    }
}

/// Register all `seq` types in [Context].
pub fn register(ctx: &mut Context) {
    ClockType::register(ctx);
    ResetType::register(ctx);
    MemoryType::register(ctx);
}

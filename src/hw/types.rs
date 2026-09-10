// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Hardware types defined in the `hw` dialect.

use pliron::{
    context::Context,
    derive::pliron_type,
    r#type::{Type, TypeHandle, TypedHandle},
};

/// Arbitrary-width bitvector hardware wire / bus type: `hw.int<width>`.
#[pliron_type(
    name = "hw.int",
    format = "`<` $width `>`",
    verifier = "succ",
    generate_get = true
)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IntType {
    width: u32,
}

impl IntType {
    /// Bitwidth of the integer wire / bus.
    pub fn width(&self) -> u32 {
        self.width
    }
}

/// Inout / bidirectional port type: `hw.inout<element_type>`.
#[pliron_type(
    name = "hw.inout",
    format = "`<` $element_type `>`",
    verifier = "succ",
    generate_get = true
)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InoutType {
    element_type: TypeHandle,
}

impl InoutType {
    /// Underlying element type driven or sampled by this inout wire.
    pub fn element_type(&self) -> TypeHandle {
        self.element_type
    }
}

/// Fixed-size packed hardware array: `hw.array<size x element_type>`.
#[pliron_type(
    name = "hw.array",
    format = "`<` $size `x` $element_type `>`",
    verifier = "succ",
    generate_get = true
)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArrayType {
    size: u64,
    element_type: TypeHandle,
}

impl ArrayType {
    /// Number of elements in the hardware array.
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Element type of each array entry.
    pub fn element_type(&self) -> TypeHandle {
        self.element_type
    }
}

/// Register all types in the `hw` dialect.
pub fn register(ctx: &mut Context) {
    IntType::register(ctx);
    InoutType::register(ctx);
    ArrayType::register(ctx);
}

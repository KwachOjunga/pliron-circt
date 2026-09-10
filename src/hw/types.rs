// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Hardware types defined in the `hw` dialect.
//!
//! Provides parity with CIRCT / MLIR `hw` dialect type systems:
//! - [`IntType`]: Arbitrary-width bitvector hardware wire/bus (`hw.int<width>`).
//! - [`InoutType`]: Bidirectional / inout wire or port (`hw.inout<element_type>`).
//! - [`ArrayType`]: Packed fixed-size multidimensional hardware array (`hw.array<size x element_type>`).
//! - [`StructType`]: Hardware record / struct of named fields (`hw.struct<...>`).
//! - [`UnionType`]: Hardware union of named fields sharing storage (`hw.union<...>`).
//! - [`TypeAliasType`]: Symbolic type alias referencing a `hw.typedecl` (`hw.typealias<@symbol, inner_type>`).
//! - [`ModuleType`]: First-class module interface signature (`hw.module_type<...>`).

use pliron::{
    context::Context,
    derive::pliron_type,
    identifier::Identifier,
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

/// Hardware struct / record type: `hw.struct<fieldName: FieldType, ...>`.
#[pliron_type(
    name = "hw.struct",
    verifier = "succ",
    generate_get = true
)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StructType {
    fields: Vec<(Identifier, TypeHandle)>,
}

impl StructType {
    /// Get reference to all named fields and their types.
    pub fn fields(&self) -> &[(Identifier, TypeHandle)] {
        &self.fields
    }

    /// Get the number of fields in the struct.
    pub fn num_fields(&self) -> usize {
        self.fields.len()
    }

    /// Find field index by field name.
    pub fn get_field_index(&self, name: &Identifier) -> Option<usize> {
        self.fields.iter().position(|(n, _)| n == name)
    }

    /// Find field type by field name.
    pub fn get_field_type(&self, name: &Identifier) -> Option<TypeHandle> {
        self.fields.iter().find(|(n, _)| n == name).map(|(_, ty)| *ty)
    }
}

/// Hardware union type: `hw.union<fieldName: FieldType, ...>`.
#[pliron_type(
    name = "hw.union",
    verifier = "succ",
    generate_get = true
)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnionType {
    fields: Vec<(Identifier, TypeHandle)>,
}

impl UnionType {
    /// Get reference to all union variant fields.
    pub fn fields(&self) -> &[(Identifier, TypeHandle)] {
        &self.fields
    }

    /// Get the number of fields in the union.
    pub fn num_fields(&self) -> usize {
        self.fields.len()
    }
}

/// Symbolic type alias referencing a `hw.typedecl`: `hw.typealias<@symbol, inner_type>`.
#[pliron_type(
    name = "hw.typealias",
    verifier = "succ",
    generate_get = true
)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeAliasType {
    symbol: Identifier,
    inner_type: TypeHandle,
}

impl TypeAliasType {
    /// Identifier symbol of the aliased type.
    pub fn symbol(&self) -> &Identifier {
        &self.symbol
    }

    /// Inner underlying hardware type.
    pub fn inner_type(&self) -> TypeHandle {
        self.inner_type
    }
}

/// First-class hardware module interface signature type: `hw.module_type<in (...), out (...)>`.
#[pliron_type(
    name = "hw.module_type",
    verifier = "succ",
    generate_get = true
)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModuleType {
    inputs: Vec<(Identifier, TypeHandle)>,
    outputs: Vec<(Identifier, TypeHandle)>,
}

impl ModuleType {
    /// List of input port identifiers and types.
    pub fn inputs(&self) -> &[(Identifier, TypeHandle)] {
        &self.inputs
    }

    /// List of output port identifiers and types.
    pub fn outputs(&self) -> &[(Identifier, TypeHandle)] {
        &self.outputs
    }

    /// Number of inputs in the module signature.
    pub fn num_inputs(&self) -> usize {
        self.inputs.len()
    }

    /// Number of outputs in the module signature.
    pub fn num_outputs(&self) -> usize {
        self.outputs.len()
    }
}

/// Register all types in the `hw` dialect.
pub fn register(ctx: &mut Context) {
    IntType::register(ctx);
    InoutType::register(ctx);
    ArrayType::register(ctx);
    StructType::register(ctx);
    UnionType::register(ctx);
    TypeAliasType::register(ctx);
    ModuleType::register(ctx);
}


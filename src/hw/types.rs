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
//! - [`EnumType`]: Named, explicitly encoded finite hardware type (`hw.enum<...>`).
//! - [`ModuleType`]: First-class module interface signature (`hw.module_type<...>`).

use pliron::{
    context::Context,
    derive::{format, pliron_type},
    identifier::Identifier,
    r#type::{Type, TypeHandle},
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

/// A named hardware field and its type: `name : type`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[format("$name `:` $ty")]
pub struct StructField {
    pub name: Identifier,
    pub ty: TypeHandle,
}

impl StructField {
    /// Create a new struct/union field.
    pub fn new(name: Identifier, ty: TypeHandle) -> Self {
        Self { name, ty }
    }
}

/// Hardware struct / record type: `hw.struct<fieldName: FieldType, ...>`.
#[pliron_type(
    name = "hw.struct",
    format = "`<` vec($fields, CharSpace(`,`)) `>`",
    verifier = "succ",
    generate_get = true
)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StructType {
    fields: Vec<StructField>,
}

impl StructType {
    /// Get reference to all named fields and their types.
    pub fn fields(&self) -> &[StructField] {
        &self.fields
    }

    /// Get the number of fields in the struct.
    pub fn num_fields(&self) -> usize {
        self.fields.len()
    }

    /// Find field index by field name.
    pub fn get_field_index(&self, name: &Identifier) -> Option<usize> {
        self.fields.iter().position(|f| &f.name == name)
    }

    /// Find field type by field name.
    pub fn get_field_type(&self, name: &Identifier) -> Option<TypeHandle> {
        self.fields.iter().find(|f| &f.name == name).map(|f| f.ty)
    }
}

/// Hardware union type: `hw.union<fieldName: FieldType, ...>`.
#[pliron_type(
    name = "hw.union",
    format = "`<` vec($fields, CharSpace(`,`)) `>`",
    verifier = "succ",
    generate_get = true
)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnionType {
    fields: Vec<StructField>,
}

impl UnionType {
    /// Get reference to all union variant fields.
    pub fn fields(&self) -> &[StructField] {
        &self.fields
    }

    /// Get the number of fields in the union.
    pub fn num_fields(&self) -> usize {
        self.fields.len()
    }

    /// Find field index by field name.
    pub fn get_field_index(&self, name: &Identifier) -> Option<usize> {
        self.fields.iter().position(|f| &f.name == name)
    }

    /// Find field type by field name.
    pub fn get_field_type(&self, name: &Identifier) -> Option<TypeHandle> {
        self.fields.iter().find(|f| &f.name == name).map(|f| f.ty)
    }
}

/// Symbolic type alias referencing a `hw.typedecl`: `hw.typealias<@symbol, inner_type>`.
#[pliron_type(
    name = "hw.typealias",
    format = "`<` $symbol `,` $inner_type `>`",
    verifier = "succ",
    generate_get = true
)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeAliasType {
    symbol: Identifier,
    inner_type: TypeHandle,
}

/// A named enum variant and its encoded value.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[format("$name `=` $value")]
pub struct EnumVariant {
    pub name: Identifier,
    pub value: u64,
}

impl EnumVariant {
    /// Create a variant with an explicit underlying integer encoding.
    pub fn new(name: Identifier, value: u64) -> Self {
        Self { name, value }
    }
}

/// Named finite hardware type with explicit integer encodings: `hw.enum<...>`.
///
/// The underlying type determines the storage width. Variant encodings are
/// part of the type contract and must fit in that width; an enum value is not
/// interchangeable with an arbitrary integer without an explicit conversion.
#[pliron_type(
    name = "hw.enum",
    format = "`<` $name `:` $underlying_type `,` vec($variants, CharSpace(`,`)) `>`",
    verifier = "succ",
    generate_get = true
)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EnumType {
    name: Identifier,
    underlying_type: TypeHandle,
    variants: Vec<EnumVariant>,
}

impl EnumType {
    /// Symbolic name of this enum.
    pub fn name(&self) -> &Identifier {
        &self.name
    }

    /// Integer type used to encode enum values.
    pub fn underlying_type(&self) -> TypeHandle {
        self.underlying_type
    }

    /// All variants in declaration order.
    pub fn variants(&self) -> &[EnumVariant] {
        &self.variants
    }

    /// Find a variant by name.
    pub fn get_variant(&self, name: &Identifier) -> Option<&EnumVariant> {
        self.variants.iter().find(|variant| &variant.name == name)
    }
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
    format = "`<` `in` `(` vec($inputs, CharSpace(`,`)) `)` `,` `out` `(` vec($outputs, CharSpace(`,`)) `)` `>`",
    verifier = "succ",
    generate_get = true
)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModuleType {
    inputs: Vec<StructField>,
    outputs: Vec<StructField>,
}

impl ModuleType {
    /// List of input port identifiers and types.
    pub fn inputs(&self) -> &[StructField] {
        &self.inputs
    }

    /// List of output port identifiers and types.
    pub fn outputs(&self) -> &[StructField] {
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
    EnumType::register(ctx);
    ModuleType::register(ctx);
}

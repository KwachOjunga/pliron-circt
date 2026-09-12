# HardwareLayout Interface Specification & Design

## 1. Overview & Motivation

In hardware intermediate representations, operations frequently require compile-time queries regarding the physical layout of types:
- **Bit Conservation**: Operations like `hw.bitcast` must verify that the source and destination types have the exact same total bitwidth without emitting runtime checks.
- **Concatenation & Slicing**: Operations like `comb.concat` and `comb.extract` must verify that operand widths sum up to the result width, and that extraction ranges do not exceed the boundaries of the carrier signal.
- **Aggregate Indexing**: Flattening records (`hw.struct`), indexing arrays (`hw.array`), and accessing tagged unions (`hw.union`) require deterministic byte/bit offset calculations and field-size calculations.

Currently, in `pliron-hw`, these layout queries are either duplicated ad-hoc across operations or left unchecked (`verifier = "succ"`). 

The **`HardwareLayout`** interface establishes a uniform, queryable semantic contract for all hardware types in `pliron-hw`.

---

## 2. Abstraction Level & Semantic Contract

The `HardwareLayout` interface models the **physical, packed bit-level representation** of hardware types as they would be laid out on physical wires or in register storage.

### Core Guarantees:
1. **Determinism**: The layout of a hardware type depends strictly on its type parameters and the layout of its constituent subtypes.
2. **Dense Packing**: By default, hardware types in the `hw` dialect use dense packing (0 padding bits between array elements or struct fields) unless an explicit alignment/padding attribute is specified.
3. **Little-Endian Bit Numbering**: Bit 0 denotes the Least Significant Bit (LSB). When an aggregate is flattened into a bitvector:
   - For an array `!hw.array<N x T>`, element `0` occupies the lowest bit range `[0, width(T) - 1]`.
   - For a struct `!hw.struct<f0: T0, f1: T1>`, field `f0` occupies the highest bit range, and the last declared field occupies the lowest bit range (CIRCT / SystemVerilog packed struct convention), or vice versa as declared by the dialect configuration.

---

## 3. Rust Trait Definition

The interface is defined in `pliron-hw/src/hw/layout.rs`:

```rust
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Hardware layout queries for types in the hardware dialect family.

use pliron::{
    context::Context,
    location::Location,
    r#type::{Type, TypeHandle},
    result::Result,
    verify_err,
};

/// Information about a subfield or element within a composite hardware type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldLayout {
    /// Offset in bits from the LSB (bit 0) of the enclosing aggregate.
    pub bit_offset: u64,
    /// Width in bits of the field.
    pub bit_width: u64,
    /// The TypeHandle of the field.
    pub field_type: TypeHandle,
}

/// Interface implemented by types that have a well-defined hardware physical layout.
pub trait HardwareLayout {
    /// Total flattened width of this type in bits.
    fn bitwidth(&self, ctx: &Context) -> Result<u64>;

    /// Returns `true` if this type is a ground type (primitive wire/bus/clock)
    /// rather than an aggregate (array/struct/union).
    fn is_ground(&self, ctx: &Context) -> bool;

    /// Returns the number of direct subfields or elements (0 for ground types).
    fn num_elements(&self, ctx: &Context) -> usize;

    /// Computes the bit offset and width of a subfield by index.
    fn get_field_layout(&self, ctx: &Context, index: usize) -> Result<FieldLayout>;
}
```

---

## 4. Implementations for Hardware Types

### 4.1 `IntType` (`hw.int<width>`)
- `bitwidth`: Exactly `self.width() as u64`.
- `is_ground`: `true`.
- `num_elements`: `0`.
- `get_field_layout`: Returns error (ground type).

### 4.2 `ArrayType` (`hw.array<size x element_type>`)
- `bitwidth`: $\text{size} \times \text{bitwidth}(\text{element\_type})$.
- `is_ground`: `false`.
- `num_elements`: `self.size() as usize`.
- `get_field_layout(i)`:
  - $\text{bit\_offset} = i \times \text{elem\_width}$
  - $\text{bit\_width} = \text{elem\_width}$

### 4.3 `StructType` (`hw.struct<f0: T0, f1: T1, ...>`)
- `bitwidth`: $\sum_{i=0}^{N-1} \text{bitwidth}(T_i)$.
- `is_ground`: `false`.
- `num_elements`: Number of fields.
- `get_field_layout(i)`: Computes cumulative bit offsets based on packing order.

### 4.4 `UnionType` (`hw.union<f0: T0, f1: T1, ...>`)
- `bitwidth`: $\max_{i=0}^{N-1} (\text{bitwidth}(T_i))$.
- `is_ground`: `false`.
- `num_elements`: Number of variants.
- `get_field_layout(i)`: $\text{bit\_offset} = 0$, $\text{bit\_width} = \text{bitwidth}(T_i)$.

### 4.5 `EnumType` (`hw.enum<Name: iN, ...>`)
- `bitwidth`: Bitwidth of `self.underlying_type()`.
- `is_ground`: `true`.
- `num_elements`: `0`.

---

## 5. Integration with Operation Verifiers

### 1. `hw.bitcast`
Reinterpretation between types of identical total bitwidth:
```rust
let src_width = src_ty.bitwidth(ctx)?;
let dst_width = dst_ty.bitwidth(ctx)?;
if src_width != dst_width {
    return verify_err!(
        op.loc(),
        "hw.bitcast requires source and target types to have identical bitwidths (got {} vs {})",
        src_width, dst_width
    );
}
```

### 2. `comb.concat`
Bitvector concatenation:
```rust
let mut total_operand_width = 0;
for operand in op.get_operands() {
    total_operand_width += operand.get_type(ctx).bitwidth(ctx)?;
}
let res_width = op.get_result(0).get_type(ctx).bitwidth(ctx)?;
if total_operand_width != res_width {
    return verify_err!(
        op.loc(),
        "comb.concat operands sum to {} bits, but result type expects {} bits",
        total_operand_width, res_width
    );
}
```

### 3. `comb.extract`
Bit-slice extraction:
```rust
let input_width = input_ty.bitwidth(ctx)?;
let result_width = result_ty.bitwidth(ctx)?;
if low_bit + result_width > input_width {
    return verify_err!(
        op.loc(),
        "comb.extract range [{}..{}] exceeds input width ({})",
        low_bit, low_bit + result_width, input_width
    );
}
```

---

## 6. Testing Strategy

1. **Unit Tests (`tests/layout_tests.rs`)**:
   - Verify that nested aggregates (e.g. `!hw.array<4 x !hw.struct<a: i8, b: i16>>`) correctly report $4 \times (8 + 16) = 96$ bits.
   - Verify that unions report the maximum variant width.
2. **Verifier Tests**:
   - Verify that `hw.bitcast` rejects mismatches.
   - Verify that `comb.concat` rejects width mismatches.

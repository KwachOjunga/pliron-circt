// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Operations defined in the `comb` (Combinational Logic) dialect.

use pliron::{
    builtin::{
        attributes::{IntegerAttr, StringAttr},
        op_interfaces::{NOpdsInterface, NRegionsInterface, OneResultInterface},
    },
    context::Context,
    derive::pliron_op,
    op::Op,
    operation::Operation,
    r#type::TypeHandle,
    value::Value,
};

/// Comparison predicate for `comb.icmp`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ICmpPredicate {
    EQ,
    NE,
    SLT,
    SLE,
    SGT,
    SGE,
    ULT,
    ULE,
    UGT,
    UGE,
}

impl ICmpPredicate {
    /// Return predicate as a standard string identifier.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EQ => "eq",
            Self::NE => "ne",
            Self::SLT => "slt",
            Self::SLE => "sle",
            Self::SGT => "sgt",
            Self::SGE => "sge",
            Self::ULT => "ult",
            Self::ULE => "ule",
            Self::UGT => "ugt",
            Self::UGE => "uge",
        }
    }

    /// Parse predicate from string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "eq" => Some(Self::EQ),
            "ne" => Some(Self::NE),
            "slt" => Some(Self::SLT),
            "sle" => Some(Self::SLE),
            "sgt" => Some(Self::SGT),
            "sge" => Some(Self::SGE),
            "ult" => Some(Self::ULT),
            "ule" => Some(Self::ULE),
            "ugt" => Some(Self::UGT),
            "uge" => Some(Self::UGE),
            _ => None,
        }
    }
}

// -----------------------------------------------------------------------------
// Arithmetic Operations
// -----------------------------------------------------------------------------

/// Addition operation: `comb.add`
#[pliron_op(
    name = "comb.add",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<2>],
    verifier = "succ",
)]
pub struct AddOp;

impl AddOp {
    /// Create a new `comb.add`.
    pub fn new(ctx: &mut Context, lhs: Value, rhs: Value, res_ty: TypeHandle) -> Self {
        let op = Operation::new(ctx, Self::get_concrete_op_info(), vec![res_ty], vec![lhs, rhs], vec![], 0);
        AddOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Subtraction operation: `comb.sub`
#[pliron_op(
    name = "comb.sub",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<2>],
    verifier = "succ",
)]
pub struct SubOp;

impl SubOp {
    /// Create a new `comb.sub`.
    pub fn new(ctx: &mut Context, lhs: Value, rhs: Value, res_ty: TypeHandle) -> Self {
        let op = Operation::new(ctx, Self::get_concrete_op_info(), vec![res_ty], vec![lhs, rhs], vec![], 0);
        SubOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Multiplication operation: `comb.mul`
#[pliron_op(
    name = "comb.mul",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<2>],
    verifier = "succ",
)]
pub struct MulOp;

impl MulOp {
    /// Create a new `comb.mul`.
    pub fn new(ctx: &mut Context, lhs: Value, rhs: Value, res_ty: TypeHandle) -> Self {
        let op = Operation::new(ctx, Self::get_concrete_op_info(), vec![res_ty], vec![lhs, rhs], vec![], 0);
        MulOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Unsigned division operation: `comb.divu`
#[pliron_op(
    name = "comb.divu",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<2>],
    verifier = "succ",
)]
pub struct DivUOp;

impl DivUOp {
    /// Create a new `comb.divu`.
    pub fn new(ctx: &mut Context, lhs: Value, rhs: Value, res_ty: TypeHandle) -> Self {
        let op = Operation::new(ctx, Self::get_concrete_op_info(), vec![res_ty], vec![lhs, rhs], vec![], 0);
        DivUOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Signed division operation: `comb.divs`
#[pliron_op(
    name = "comb.divs",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<2>],
    verifier = "succ",
)]
pub struct DivSOp;

impl DivSOp {
    /// Create a new `comb.divs`.
    pub fn new(ctx: &mut Context, lhs: Value, rhs: Value, res_ty: TypeHandle) -> Self {
        let op = Operation::new(ctx, Self::get_concrete_op_info(), vec![res_ty], vec![lhs, rhs], vec![], 0);
        DivSOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Unsigned modulo remainder operation: `comb.modu`
#[pliron_op(
    name = "comb.modu",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<2>],
    verifier = "succ",
)]
pub struct ModUOp;

impl ModUOp {
    /// Create a new `comb.modu`.
    pub fn new(ctx: &mut Context, lhs: Value, rhs: Value, res_ty: TypeHandle) -> Self {
        let op = Operation::new(ctx, Self::get_concrete_op_info(), vec![res_ty], vec![lhs, rhs], vec![], 0);
        ModUOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Signed modulo remainder operation: `comb.mods`
#[pliron_op(
    name = "comb.mods",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<2>],
    verifier = "succ",
)]
pub struct ModSOp;

impl ModSOp {
    /// Create a new `comb.mods`.
    pub fn new(ctx: &mut Context, lhs: Value, rhs: Value, res_ty: TypeHandle) -> Self {
        let op = Operation::new(ctx, Self::get_concrete_op_info(), vec![res_ty], vec![lhs, rhs], vec![], 0);
        ModSOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Shift left operation: `comb.shl`
#[pliron_op(
    name = "comb.shl",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<2>],
    verifier = "succ",
)]
pub struct ShlOp;

impl ShlOp {
    /// Create a new `comb.shl`.
    pub fn new(ctx: &mut Context, val: Value, shift: Value, res_ty: TypeHandle) -> Self {
        let op = Operation::new(ctx, Self::get_concrete_op_info(), vec![res_ty], vec![val, shift], vec![], 0);
        ShlOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Shift right unsigned (logical): `comb.shru`
#[pliron_op(
    name = "comb.shru",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<2>],
    verifier = "succ",
)]
pub struct ShrUOp;

impl ShrUOp {
    /// Create a new `comb.shru`.
    pub fn new(ctx: &mut Context, val: Value, shift: Value, res_ty: TypeHandle) -> Self {
        let op = Operation::new(ctx, Self::get_concrete_op_info(), vec![res_ty], vec![val, shift], vec![], 0);
        ShrUOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Shift right signed (arithmetic): `comb.shrs`
#[pliron_op(
    name = "comb.shrs",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<2>],
    verifier = "succ",
)]
pub struct ShrSOp;

impl ShrSOp {
    /// Create a new `comb.shrs`.
    pub fn new(ctx: &mut Context, val: Value, shift: Value, res_ty: TypeHandle) -> Self {
        let op = Operation::new(ctx, Self::get_concrete_op_info(), vec![res_ty], vec![val, shift], vec![], 0);
        ShrSOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

// -----------------------------------------------------------------------------
// Bitwise & Logical Operations
// -----------------------------------------------------------------------------

/// Bitwise AND operation: `comb.and`
#[pliron_op(
    name = "comb.and",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface],
    verifier = "succ",
)]
pub struct AndOp;

impl AndOp {
    /// Create a new `comb.and`.
    pub fn new(ctx: &mut Context, inputs: Vec<Value>, res_ty: TypeHandle) -> Self {
        let op = Operation::new(ctx, Self::get_concrete_op_info(), vec![res_ty], inputs, vec![], 0);
        AndOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Bitwise OR operation: `comb.or`
#[pliron_op(
    name = "comb.or",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface],
    verifier = "succ",
)]
pub struct OrOp;

impl OrOp {
    /// Create a new `comb.or`.
    pub fn new(ctx: &mut Context, inputs: Vec<Value>, res_ty: TypeHandle) -> Self {
        let op = Operation::new(ctx, Self::get_concrete_op_info(), vec![res_ty], inputs, vec![], 0);
        OrOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Bitwise XOR operation: `comb.xor`
#[pliron_op(
    name = "comb.xor",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface],
    verifier = "succ",
)]
pub struct XorOp;

impl XorOp {
    /// Create a new `comb.xor`.
    pub fn new(ctx: &mut Context, inputs: Vec<Value>, res_ty: TypeHandle) -> Self {
        let op = Operation::new(ctx, Self::get_concrete_op_info(), vec![res_ty], inputs, vec![], 0);
        XorOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

// -----------------------------------------------------------------------------
// Multiplexer & Comparison
// -----------------------------------------------------------------------------

/// 2-to-1 Multiplexer operation: `comb.mux`
#[pliron_op(
    name = "comb.mux",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<3>],
    verifier = "succ",
)]
pub struct MuxOp;

impl MuxOp {
    /// Create a new `comb.mux`.
    ///
    /// Semantics: `cond ? true_val : false_val`.
    pub fn new(ctx: &mut Context, cond: Value, true_val: Value, false_val: Value, res_ty: TypeHandle) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![res_ty],
            vec![cond, true_val, false_val],
            vec![],
            0,
        );
        MuxOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Integer comparison operation: `comb.icmp`
#[pliron_op(
    name = "comb.icmp",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<2>],
    attributes = (predicate: StringAttr),
    verifier = "succ",
)]
pub struct ICmpOp;

impl ICmpOp {
    /// Create a new `comb.icmp`.
    pub fn new(
        ctx: &mut Context,
        predicate: ICmpPredicate,
        lhs: Value,
        rhs: Value,
        i1_ty: TypeHandle,
    ) -> Self {
        let op = Operation::new(ctx, Self::get_concrete_op_info(), vec![i1_ty], vec![lhs, rhs], vec![], 0);
        let icmp = ICmpOp { op };
        icmp.set_attr_predicate(ctx, predicate.as_str().to_string().into());
        icmp
    }

    /// Get comparison result (`i1`).
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

// -----------------------------------------------------------------------------
// Bit Manipulations
// -----------------------------------------------------------------------------

/// Bit concatenation operation: `comb.concat`
#[pliron_op(
    name = "comb.concat",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface],
    verifier = "succ",
)]
pub struct ConcatOp;

impl ConcatOp {
    /// Create a new `comb.concat`.
    pub fn new(ctx: &mut Context, inputs: Vec<Value>, res_ty: TypeHandle) -> Self {
        let op = Operation::new(ctx, Self::get_concrete_op_info(), vec![res_ty], inputs, vec![], 0);
        ConcatOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Bit extraction operation: `comb.extract`
#[pliron_op(
    name = "comb.extract",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<1>],
    attributes = (extract_low_bit: IntegerAttr),
    verifier = "succ",
)]
pub struct ExtractOp;

impl ExtractOp {
    /// Create a new `comb.extract`.
    pub fn new(ctx: &mut Context, val: Value, low_bit: IntegerAttr, res_ty: TypeHandle) -> Self {
        let op = Operation::new(ctx, Self::get_concrete_op_info(), vec![res_ty], vec![val], vec![], 0);
        let extract = ExtractOp { op };
        extract.set_attr_extract_low_bit(ctx, low_bit);
        extract
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Bit replication operation: `comb.replicate`
#[pliron_op(
    name = "comb.replicate",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<1>],
    attributes = (count: IntegerAttr),
    verifier = "succ",
)]
pub struct ReplicateOp;

impl ReplicateOp {
    /// Create a new `comb.replicate`.
    pub fn new(ctx: &mut Context, val: Value, count: IntegerAttr, res_ty: TypeHandle) -> Self {
        let op = Operation::new(ctx, Self::get_concrete_op_info(), vec![res_ty], vec![val], vec![], 0);
        let rep = ReplicateOp { op };
        rep.set_attr_count(ctx, count);
        rep
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Parity reduction operation: `comb.parity`
#[pliron_op(
    name = "comb.parity",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<1>],
    verifier = "succ",
)]
pub struct ParityOp;

impl ParityOp {
    /// Create a new `comb.parity`.
    pub fn new(ctx: &mut Context, val: Value, i1_ty: TypeHandle) -> Self {
        let op = Operation::new(ctx, Self::get_concrete_op_info(), vec![i1_ty], vec![val], vec![], 0);
        ParityOp { op }
    }

    /// Get result value (`i1`).
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Register all `comb` operations in [Context].
pub fn register(ctx: &mut Context) {
    AddOp::register(ctx);
    SubOp::register(ctx);
    MulOp::register(ctx);
    DivUOp::register(ctx);
    DivSOp::register(ctx);
    ModUOp::register(ctx);
    ModSOp::register(ctx);
    ShlOp::register(ctx);
    ShrUOp::register(ctx);
    ShrSOp::register(ctx);
    AndOp::register(ctx);
    OrOp::register(ctx);
    XorOp::register(ctx);
    MuxOp::register(ctx);
    ICmpOp::register(ctx);
    ConcatOp::register(ctx);
    ExtractOp::register(ctx);
    ReplicateOp::register(ctx);
    ParityOp::register(ctx);
}

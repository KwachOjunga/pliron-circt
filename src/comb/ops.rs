// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Operations defined in the `comb` (Combinational Logic) dialect.
//!
//! Every operation is pure, side-effect-free, zero-latency combinational logic.
//! Operands and results must have valid non-zero signless integer types.

use pliron::{
    builtin::{
        attributes::{IntegerAttr, StringAttr},
        op_interfaces::{NOpdsInterface, NRegionsInterface, OneResultInterface},
    },
    common_traits::Verify,
    context::Context,
    derive::{op_interface_impl, pliron_op},
    location::Located,
    op::Op,
    operation::Operation,
    opts::dce::SideEffects,
    printable::Printable,
    result::Result,
    r#type::{TypeHandle, Typed},
    value::Value,
    verify_err,
};

use super::types::{CombOpExt, verify_i1, verify_integer_type};
use super::utils::*;

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

    #[allow(clippy::should_implement_trait)]
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

    /// Whether this comparison predicate is signed.
    pub fn is_signed(&self) -> bool {
        matches!(self, Self::SLT | Self::SLE | Self::SGT | Self::SGE)
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
)]
pub struct AddOp;

impl Verify for AddOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        verify_binary_arithmetic(&op, ctx, "comb.add")
    }
}

#[op_interface_impl]
impl SideEffects for AddOp {
    fn has_side_effects(&self, _ctx: &Context) -> bool {
        false
    }
}

impl CombOpExt for AddOp {
    fn width(&self, ctx: &Context) -> u32 {
        verify_integer_type(
            &self.get_operation().deref(ctx),
            ctx,
            self.result(ctx).get_type(ctx),
            "res",
        )
        .unwrap_or(0)
    }
}

impl AddOp {
    /// Create a new `comb.add`.
    pub fn new(ctx: &mut Context, args: Vec<Value>, res_ty: TypeHandle) -> Self {
        assert_binary_arithmetic(ctx, &args, res_ty, "comb.add");
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![res_ty],
            args,
            vec![],
            0,
        );
        AddOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }

    /// Get lhs operand.
    pub fn lhs(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(0)
    }

    /// Get rhs operand.
    pub fn rhs(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(1)
    }
}

/// Subtraction operation: `comb.sub`
#[pliron_op(
    name = "comb.sub",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<2>],
)]
pub struct SubOp;

impl Verify for SubOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        verify_binary_arithmetic(&op, ctx, "comb.sub")
    }
}

#[op_interface_impl]
impl SideEffects for SubOp {
    fn has_side_effects(&self, _ctx: &Context) -> bool {
        false
    }
}

impl CombOpExt for SubOp {
    fn width(&self, ctx: &Context) -> u32 {
        verify_integer_type(
            &self.get_operation().deref(ctx),
            ctx,
            self.result(ctx).get_type(ctx),
            "res",
        )
        .unwrap_or(0)
    }
}

impl SubOp {
    /// Create a new `comb.sub`.
    pub fn new(ctx: &mut Context, lhs: Value, rhs: Value, res_ty: TypeHandle) -> Self {
        assert_binary_arithmetic(ctx, &[lhs, rhs], res_ty, "comb.sub");
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![res_ty],
            vec![lhs, rhs],
            vec![],
            0,
        );
        SubOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }

    /// Get lhs operand.
    pub fn lhs(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(0)
    }

    /// Get rhs operand.
    pub fn rhs(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(1)
    }
}

/// Multiplication operation: `comb.mul`
#[pliron_op(
    name = "comb.mul",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<2>],
)]
pub struct MulOp;

impl Verify for MulOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        verify_binary_arithmetic(&op, ctx, "comb.mul")
    }
}

#[op_interface_impl]
impl SideEffects for MulOp {
    fn has_side_effects(&self, _ctx: &Context) -> bool {
        false
    }
}

impl CombOpExt for MulOp {
    fn width(&self, ctx: &Context) -> u32 {
        verify_integer_type(
            &self.get_operation().deref(ctx),
            ctx,
            self.result(ctx).get_type(ctx),
            "res",
        )
        .unwrap_or(0)
    }
}

impl MulOp {
    /// Create a new `comb.mul`.
    pub fn new(ctx: &mut Context, args: Vec<Value>, res_ty: TypeHandle) -> Self {
        assert_binary_arithmetic(ctx, &args, res_ty, "comb.mul");
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![res_ty],
            args,
            vec![],
            0,
        );
        MulOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }

    /// Get lhs operand.
    pub fn lhs(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(0)
    }

    /// Get rhs operand.
    pub fn rhs(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(1)
    }
}

/// Unsigned division operation: `comb.divu`
#[pliron_op(
    name = "comb.divu",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<2>],
)]
pub struct DivUOp;

impl Verify for DivUOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        verify_binary_arithmetic(&op, ctx, "comb.divu")
    }
}

#[op_interface_impl]
impl SideEffects for DivUOp {
    fn has_side_effects(&self, _ctx: &Context) -> bool {
        false
    }
}

impl CombOpExt for DivUOp {
    fn width(&self, ctx: &Context) -> u32 {
        verify_integer_type(
            &self.get_operation().deref(ctx),
            ctx,
            self.result(ctx).get_type(ctx),
            "res",
        )
        .unwrap_or(0)
    }
}

impl DivUOp {
    /// Create a new `comb.divu`.
    pub fn new(ctx: &mut Context, lhs: Value, rhs: Value, res_ty: TypeHandle) -> Self {
        assert_binary_arithmetic(ctx, &vec![lhs, rhs], res_ty, "comb.divu");
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![res_ty],
            vec![lhs, rhs],
            vec![],
            0,
        );
        DivUOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }

    /// Get lhs operand.
    pub fn lhs(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(0)
    }

    /// Get rhs operand.
    pub fn rhs(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(1)
    }
}

/// Signed division operation: `comb.divs`
#[pliron_op(
    name = "comb.divs",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<2>],
)]
pub struct DivSOp;

impl Verify for DivSOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        verify_binary_arithmetic(&op, ctx, "comb.divs")
    }
}

#[op_interface_impl]
impl SideEffects for DivSOp {
    fn has_side_effects(&self, _ctx: &Context) -> bool {
        false
    }
}

impl CombOpExt for DivSOp {
    fn width(&self, ctx: &Context) -> u32 {
        verify_integer_type(
            &self.get_operation().deref(ctx),
            ctx,
            self.result(ctx).get_type(ctx),
            "res",
        )
        .unwrap_or(0)
    }
    fn is_signed(&self) -> bool {
        true
    }
}

impl DivSOp {
    /// Create a new `comb.divs`.
    pub fn new(ctx: &mut Context, lhs: Value, rhs: Value, res_ty: TypeHandle) -> Self {
        assert_binary_arithmetic(ctx, &[lhs, rhs], res_ty, "comb.divs");
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![res_ty],
            vec![lhs, rhs],
            vec![],
            0,
        );
        DivSOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }

    /// Get lhs operand.
    pub fn lhs(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(0)
    }

    /// Get rhs operand.
    pub fn rhs(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(1)
    }
}

/// Unsigned modulo remainder operation: `comb.modu`
#[pliron_op(
    name = "comb.modu",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<2>],
)]
pub struct ModUOp;

impl Verify for ModUOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        verify_binary_arithmetic(&op, ctx, "comb.modu")
    }
}

#[op_interface_impl]
impl SideEffects for ModUOp {
    fn has_side_effects(&self, _ctx: &Context) -> bool {
        false
    }
}

impl CombOpExt for ModUOp {
    fn width(&self, ctx: &Context) -> u32 {
        verify_integer_type(
            &self.get_operation().deref(ctx),
            ctx,
            self.result(ctx).get_type(ctx),
            "res",
        )
        .unwrap_or(0)
    }
}

impl ModUOp {
    /// Create a new `comb.modu`.
    pub fn new(ctx: &mut Context, lhs: Value, rhs: Value, res_ty: TypeHandle) -> Self {
        assert_binary_arithmetic(ctx, &[lhs, rhs], res_ty, "comb.modu");
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![res_ty],
            vec![lhs, rhs],
            vec![],
            0,
        );
        ModUOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }

    /// Get lhs operand.
    pub fn lhs(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(0)
    }

    /// Get rhs operand.
    pub fn rhs(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(1)
    }
}

/// Signed modulo remainder operation: `comb.mods`
#[pliron_op(
    name = "comb.mods",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<2>],
)]
pub struct ModSOp;

impl Verify for ModSOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        verify_binary_arithmetic(&op, ctx, "comb.mods")
    }
}

#[op_interface_impl]
impl SideEffects for ModSOp {
    fn has_side_effects(&self, _ctx: &Context) -> bool {
        false
    }
}

impl CombOpExt for ModSOp {
    fn width(&self, ctx: &Context) -> u32 {
        verify_integer_type(
            &self.get_operation().deref(ctx),
            ctx,
            self.result(ctx).get_type(ctx),
            "res",
        )
        .unwrap_or(0)
    }
    fn is_signed(&self) -> bool {
        true
    }
}

impl ModSOp {
    /// Create a new `comb.mods`.
    pub fn new(ctx: &mut Context, lhs: Value, rhs: Value, res_ty: TypeHandle) -> Self {
        assert_binary_arithmetic(ctx, &[lhs, rhs], res_ty, "comb.mods");
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![res_ty],
            vec![lhs, rhs],
            vec![],
            0,
        );
        ModSOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }

    /// Get lhs operand.
    pub fn lhs(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(0)
    }

    /// Get rhs operand.
    pub fn rhs(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(1)
    }
}

/// Shift left operation: `comb.shl`
#[pliron_op(
    name = "comb.shl",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<2>],
)]
pub struct ShlOp;

impl Verify for ShlOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        verify_binary_arithmetic(&op, ctx, "comb.shl")
    }
}

#[op_interface_impl]
impl SideEffects for ShlOp {
    fn has_side_effects(&self, _ctx: &Context) -> bool {
        false
    }
}

impl CombOpExt for ShlOp {
    fn width(&self, ctx: &Context) -> u32 {
        verify_integer_type(
            &self.get_operation().deref(ctx),
            ctx,
            self.result(ctx).get_type(ctx),
            "res",
        )
        .unwrap_or(0)
    }
}

impl ShlOp {
    /// Create a new `comb.shl`.
    pub fn new(ctx: &mut Context, val: Value, shift: Value, res_ty: TypeHandle) -> Self {
        assert_binary_arithmetic(ctx, &[val, shift], res_ty, "comb.shl");
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![res_ty],
            vec![val, shift],
            vec![],
            0,
        );
        ShlOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }

    /// Get val operand.
    pub fn val(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(0)
    }

    /// Get shift operand.
    pub fn shift(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(1)
    }
}

/// Shift right unsigned (logical): `comb.shru`
#[pliron_op(
    name = "comb.shru",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<2>],
)]
pub struct ShrUOp;

impl Verify for ShrUOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        verify_binary_arithmetic(&op, ctx, "comb.shru")
    }
}

#[op_interface_impl]
impl SideEffects for ShrUOp {
    fn has_side_effects(&self, _ctx: &Context) -> bool {
        false
    }
}

impl CombOpExt for ShrUOp {
    fn width(&self, ctx: &Context) -> u32 {
        verify_integer_type(
            &self.get_operation().deref(ctx),
            ctx,
            self.result(ctx).get_type(ctx),
            "res",
        )
        .unwrap_or(0)
    }
}

impl ShrUOp {
    /// Create a new `comb.shru`.
    pub fn new(ctx: &mut Context, val: Value, shift: Value, res_ty: TypeHandle) -> Self {
        assert_binary_arithmetic(ctx, &[val, shift], res_ty, "comb.shru");
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![res_ty],
            vec![val, shift],
            vec![],
            0,
        );
        ShrUOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }

    /// Get val operand.
    pub fn val(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(0)
    }

    /// Get shift operand.
    pub fn shift(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(1)
    }
}

/// Shift right signed (arithmetic): `comb.shrs`
#[pliron_op(
    name = "comb.shrs",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<2>],
)]
pub struct ShrSOp;

impl Verify for ShrSOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        verify_binary_arithmetic(&op, ctx, "comb.shrs")
    }
}

#[op_interface_impl]
impl SideEffects for ShrSOp {
    fn has_side_effects(&self, _ctx: &Context) -> bool {
        false
    }
}

impl CombOpExt for ShrSOp {
    fn width(&self, ctx: &Context) -> u32 {
        verify_integer_type(
            &self.get_operation().deref(ctx),
            ctx,
            self.result(ctx).get_type(ctx),
            "res",
        )
        .unwrap_or(0)
    }
    fn is_signed(&self) -> bool {
        true
    }
}

impl ShrSOp {
    /// Create a new `comb.shrs`.
    pub fn new(ctx: &mut Context, val: Value, shift: Value, res_ty: TypeHandle) -> Self {
        assert_binary_arithmetic(ctx, &[val, shift], res_ty, "comb.shrs");
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![res_ty],
            vec![val, shift],
            vec![],
            0,
        );
        ShrSOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }

    /// Get val operand.
    pub fn val(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(0)
    }

    /// Get shift operand.
    pub fn shift(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(1)
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
)]
pub struct AndOp;

impl Verify for AndOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        verify_variadic_bitwise(&op, ctx, "comb.and")
    }
}

#[op_interface_impl]
impl SideEffects for AndOp {
    fn has_side_effects(&self, _ctx: &Context) -> bool {
        false
    }
}

impl CombOpExt for AndOp {
    fn width(&self, ctx: &Context) -> u32 {
        verify_integer_type(
            &self.get_operation().deref(ctx),
            ctx,
            self.result(ctx).get_type(ctx),
            "res",
        )
        .unwrap_or(0)
    }
}

impl AndOp {
    /// Create a new `comb.and`.
    pub fn new(ctx: &mut Context, inputs: Vec<Value>, res_ty: TypeHandle) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![res_ty],
            inputs,
            vec![],
            0,
        );
        AndOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }

    /// Get all input operands.
    pub fn inputs(&self, ctx: &Context) -> Vec<Value> {
        self.get_operation().deref(ctx).operands().collect()
    }
}

/// Bitwise OR operation: `comb.or`
#[pliron_op(
    name = "comb.or",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface],
)]
pub struct OrOp;

impl Verify for OrOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        verify_variadic_bitwise(&op, ctx, "comb.or")
    }
}

#[op_interface_impl]
impl SideEffects for OrOp {
    fn has_side_effects(&self, _ctx: &Context) -> bool {
        false
    }
}

impl CombOpExt for OrOp {
    fn width(&self, ctx: &Context) -> u32 {
        verify_integer_type(
            &self.get_operation().deref(ctx),
            ctx,
            self.result(ctx).get_type(ctx),
            "res",
        )
        .unwrap_or(0)
    }
}

impl OrOp {
    /// Create a new `comb.or`.
    pub fn new(ctx: &mut Context, inputs: Vec<Value>, res_ty: TypeHandle) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![res_ty],
            inputs,
            vec![],
            0,
        );
        OrOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }

    /// Get all input operands.
    pub fn inputs(&self, ctx: &Context) -> Vec<Value> {
        self.get_operation().deref(ctx).operands().collect()
    }
}

/// Bitwise XOR operation: `comb.xor`
#[pliron_op(
    name = "comb.xor",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface],
)]
pub struct XorOp;

impl Verify for XorOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        verify_variadic_bitwise(&op, ctx, "comb.xor")
    }
}

#[op_interface_impl]
impl SideEffects for XorOp {
    fn has_side_effects(&self, _ctx: &Context) -> bool {
        false
    }
}

impl CombOpExt for XorOp {
    fn width(&self, ctx: &Context) -> u32 {
        verify_integer_type(
            &self.get_operation().deref(ctx),
            ctx,
            self.result(ctx).get_type(ctx),
            "res",
        )
        .unwrap_or(0)
    }
}

impl XorOp {
    /// Create a new `comb.xor`.
    pub fn new(ctx: &mut Context, inputs: Vec<Value>, res_ty: TypeHandle) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![res_ty],
            inputs,
            vec![],
            0,
        );
        XorOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }

    /// Get all input operands.
    pub fn inputs(&self, ctx: &Context) -> Vec<Value> {
        self.get_operation().deref(ctx).operands().collect()
    }
}

/// Bitwise complement operation: `comb.not`.
#[pliron_op(
    name = "comb.not",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<1>],
)]
pub struct NotOp;

impl Verify for NotOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        verify_unary(&op, ctx, "comb.not")
    }
}

#[op_interface_impl]
impl SideEffects for NotOp {
    fn has_side_effects(&self, _ctx: &Context) -> bool {
        false
    }
}

impl CombOpExt for NotOp {
    fn width(&self, ctx: &Context) -> u32 {
        verify_integer_type(
            &self.get_operation().deref(ctx),
            ctx,
            self.result(ctx).get_type(ctx),
            "res",
        )
        .unwrap_or(0)
    }
}

impl NotOp {
    /// Create a new `comb.not`.
    pub fn new(ctx: &mut Context, val: Value, res_ty: TypeHandle) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![res_ty],
            vec![val],
            vec![],
            0,
        );
        NotOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }

    /// Get input operand.
    pub fn val(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(0)
    }
}

/// Two's-complement negation operation: `comb.neg`.
#[pliron_op(
    name = "comb.neg",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<1>],
)]
pub struct NegOp;

impl Verify for NegOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        verify_unary(&op, ctx, "comb.neg")
    }
}

#[op_interface_impl]
impl SideEffects for NegOp {
    fn has_side_effects(&self, _ctx: &Context) -> bool {
        false
    }
}

impl CombOpExt for NegOp {
    fn width(&self, ctx: &Context) -> u32 {
        verify_integer_type(
            &self.get_operation().deref(ctx),
            ctx,
            self.result(ctx).get_type(ctx),
            "res",
        )
        .unwrap_or(0)
    }
}

impl NegOp {
    /// Create a new `comb.neg`.
    pub fn new(ctx: &mut Context, val: Value, res_ty: TypeHandle) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![res_ty],
            vec![val],
            vec![],
            0,
        );
        NegOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }

    /// Get input operand.
    pub fn val(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(0)
    }
}

/// OR reduction operation: `comb.any`.
#[pliron_op(
    name = "comb.any",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<1>],
)]
pub struct AnyOp;

impl Verify for AnyOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        verify_reduction(&op, ctx, "comb.any")
    }
}

#[op_interface_impl]
impl SideEffects for AnyOp {
    fn has_side_effects(&self, _ctx: &Context) -> bool {
        false
    }
}

impl CombOpExt for AnyOp {
    fn width(&self, _ctx: &Context) -> u32 {
        1
    }
}

impl AnyOp {
    /// Create a new `comb.any`, producing one for any set input bit.
    pub fn new(ctx: &mut Context, val: Value, i1_ty: TypeHandle) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![i1_ty],
            vec![val],
            vec![],
            0,
        );
        AnyOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }

    /// Get input operand.
    pub fn val(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(0)
    }
}

/// AND reduction operation: `comb.all`.
#[pliron_op(
    name = "comb.all",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<1>],
)]
pub struct AllOp;

impl Verify for AllOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        verify_reduction(&op, ctx, "comb.all")
    }
}

#[op_interface_impl]
impl SideEffects for AllOp {
    fn has_side_effects(&self, _ctx: &Context) -> bool {
        false
    }
}

impl CombOpExt for AllOp {
    fn width(&self, _ctx: &Context) -> u32 {
        1
    }
}

impl AllOp {
    /// Create a new `comb.all`, producing one only when every input bit is set.
    pub fn new(ctx: &mut Context, val: Value, i1_ty: TypeHandle) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![i1_ty],
            vec![val],
            vec![],
            0,
        );
        AllOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }

    /// Get input operand.
    pub fn val(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(0)
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
)]
pub struct MuxOp;

impl Verify for MuxOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        verify_i1(
            &op,
            ctx,
            op.get_operand(0).get_type(ctx),
            "comb.mux condition",
        )?;
        let true_ty = op.get_operand(1).get_type(ctx);
        let false_ty = op.get_operand(2).get_type(ctx);
        let res_ty = op.get_result(0).get_type(ctx);
        if true_ty != false_ty {
            return verify_err!(
                op.loc(),
                "comb.mux branch type mismatch: true_val has type {}, false_val has type {}",
                true_ty.disp(ctx),
                false_ty.disp(ctx)
            );
        }
        if res_ty != true_ty {
            return verify_err!(
                op.loc(),
                "comb.mux result type mismatch: expected branch type {}, found {}",
                true_ty.disp(ctx),
                res_ty.disp(ctx)
            );
        }
        Ok(())
    }
}

#[op_interface_impl]
impl SideEffects for MuxOp {
    fn has_side_effects(&self, _ctx: &Context) -> bool {
        false
    }
}

impl CombOpExt for MuxOp {
    fn width(&self, ctx: &Context) -> u32 {
        verify_integer_type(
            &self.get_operation().deref(ctx),
            ctx,
            self.result(ctx).get_type(ctx),
            "res",
        )
        .unwrap_or(0)
    }
}

impl MuxOp {
    /// Create a new `comb.mux`.
    ///
    /// Semantics: `cond ? true_val : false_val`.
    pub fn new(
        ctx: &mut Context,
        cond: Value,
        true_val: Value,
        false_val: Value,
        res_ty: TypeHandle,
    ) -> Self {
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

    /// Get condition operand.
    pub fn cond(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(0)
    }

    /// Get true branch operand.
    pub fn true_val(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(1)
    }

    /// Get false branch operand.
    pub fn false_val(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(2)
    }
}

/// Integer comparison operation: `comb.icmp`
#[pliron_op(
    name = "comb.icmp",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<2>],
    attributes = (predicate: StringAttr),
)]
pub struct ICmpOp;

impl Verify for ICmpOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let pred_attr = self
            .get_attr_predicate(ctx)
            .ok_or_else(|| -> Result<()> {
                verify_err!(op.loc(), "comb.icmp missing required attribute predicate",)
            })
            .unwrap();
        let pred_str = pred_attr.as_ref();
        if ICmpPredicate::from_str(pred_str).is_none() {
            return verify_err!(op.loc(), "comb.icmp unknown predicate '{}'", pred_str);
        }
        let lhs_w =
            verify_integer_type(&op, ctx, op.get_operand(0).get_type(ctx), "comb.icmp lhs")?;
        let rhs_w =
            verify_integer_type(&op, ctx, op.get_operand(1).get_type(ctx), "comb.icmp rhs")?;
        if lhs_w != rhs_w {
            return verify_err!(
                op.loc(),
                "comb.icmp operand width mismatch: lhs has width {}, rhs has width {}",
                lhs_w,
                rhs_w
            );
        }
        verify_i1(&op, ctx, op.get_result(0).get_type(ctx), "comb.icmp result")
    }
}

#[op_interface_impl]
impl SideEffects for ICmpOp {
    fn has_side_effects(&self, _ctx: &Context) -> bool {
        false
    }
}

impl CombOpExt for ICmpOp {
    fn width(&self, _ctx: &Context) -> u32 {
        1
    }
    fn is_signed(&self) -> bool {
        // We can't access ctx without deref, so default check is handled via predicate
        false
    }
}

impl ICmpOp {
    /// Create a new `comb.icmp`.
    pub fn new(
        ctx: &mut Context,
        predicate: ICmpPredicate,
        lhs: Value,
        rhs: Value,
        i1_ty: TypeHandle,
    ) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![i1_ty],
            vec![lhs, rhs],
            vec![],
            0,
        );
        let icmp = ICmpOp { op };
        icmp.set_attr_predicate(ctx, predicate.as_str().to_string().into());
        icmp
    }

    /// Get comparison result (`i1`).
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }

    /// Get lhs operand.
    pub fn lhs(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(0)
    }

    /// Get rhs operand.
    pub fn rhs(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(1)
    }

    /// Get comparison predicate.
    pub fn predicate(&self, ctx: &Context) -> ICmpPredicate {
        let pred_str = self
            .get_attr_predicate(ctx)
            .expect("verified predicate attribute")
            .as_ref()
            .to_string();
        ICmpPredicate::from_str(&pred_str).expect("verified predicate")
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
)]
pub struct ConcatOp;

impl Verify for ConcatOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        if op.get_num_operands() == 0 {
            return verify_err!(
                op.loc(),
                "comb.concat requires at least one operand, found 0"
            );
        }
        let mut sum_width = 0u64;
        for (i, opd) in op.operands().enumerate() {
            let w = verify_integer_type(
                &op,
                ctx,
                opd.get_type(ctx),
                &format!("comb.concat operand {}", i),
            )?;
            sum_width += w as u64;
        }
        let res_w = verify_integer_type(
            &op,
            ctx,
            op.get_result(0).get_type(ctx),
            "comb.concat result",
        )? as u64;
        if sum_width != res_w {
            return verify_err!(
                op.loc(),
                "comb.concat result width mismatch: expected sum of operand widths {}, found {}",
                sum_width,
                res_w
            );
        }
        Ok(())
    }
}

#[op_interface_impl]
impl SideEffects for ConcatOp {
    fn has_side_effects(&self, _ctx: &Context) -> bool {
        false
    }
}

impl CombOpExt for ConcatOp {
    fn width(&self, ctx: &Context) -> u32 {
        verify_integer_type(
            &self.get_operation().deref(ctx),
            ctx,
            self.result(ctx).get_type(ctx),
            "res",
        )
        .unwrap_or(0)
    }
}

impl ConcatOp {
    /// Create a new `comb.concat`.
    pub fn new(ctx: &mut Context, inputs: Vec<Value>, res_ty: TypeHandle) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![res_ty],
            inputs,
            vec![],
            0,
        );
        ConcatOp { op }
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }

    /// Get all inputs.
    pub fn inputs(&self, ctx: &Context) -> Vec<Value> {
        self.get_operation().deref(ctx).operands().collect()
    }
}

/// Bit extraction operation: `comb.extract`
#[pliron_op(
    name = "comb.extract",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<1>],
    attributes = (extract_low_bit: IntegerAttr),
)]
pub struct ExtractOp;

impl Verify for ExtractOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let in_w = verify_integer_type(
            &op,
            ctx,
            op.get_operand(0).get_type(ctx),
            "comb.extract input",
        )?;
        let res_w = verify_integer_type(
            &op,
            ctx,
            op.get_result(0).get_type(ctx),
            "comb.extract result",
        )?;
        let low_bit_attr = self
            .get_attr_extract_low_bit(ctx)
            .ok_or_else(|| -> Result<()> {
                verify_err!(
                    op.loc(),
                    "comb.extract missing required attribute extract_low_bit",
                )
            })
            .unwrap();
        let low_bit = low_bit_attr.value().to_u64();
        if low_bit + (res_w as u64) > (in_w as u64) {
            return verify_err!(
                op.loc(),
                "comb.extract slice out of bounds: low_bit ({}) + result width ({}) exceeds input width ({})",
                low_bit,
                res_w,
                in_w
            );
        }
        Ok(())
    }
}

#[op_interface_impl]
impl SideEffects for ExtractOp {
    fn has_side_effects(&self, _ctx: &Context) -> bool {
        false
    }
}

impl CombOpExt for ExtractOp {
    fn width(&self, ctx: &Context) -> u32 {
        verify_integer_type(
            &self.get_operation().deref(ctx),
            ctx,
            self.result(ctx).get_type(ctx),
            "res",
        )
        .unwrap_or(0)
    }
}

impl ExtractOp {
    /// Create a new `comb.extract`.
    pub fn new(ctx: &mut Context, val: Value, low_bit: IntegerAttr, res_ty: TypeHandle) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![res_ty],
            vec![val],
            vec![],
            0,
        );
        let extract = ExtractOp { op };
        extract.set_attr_extract_low_bit(ctx, low_bit);
        extract
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }

    /// Get input value.
    pub fn val(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(0)
    }

    /// Get low bit attribute.
    pub fn low_bit(&self, ctx: &Context) -> IntegerAttr {
        self.get_attr_extract_low_bit(ctx)
            .expect("verified extract_low_bit")
            .clone()
    }
}

/// Bit replication operation: `comb.replicate`
#[pliron_op(
    name = "comb.replicate",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<1>],
    attributes = (count: IntegerAttr),
)]
pub struct ReplicateOp;

impl Verify for ReplicateOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let in_w = verify_integer_type(
            &op,
            ctx,
            op.get_operand(0).get_type(ctx),
            "comb.replicate input",
        )?;
        let res_w = verify_integer_type(
            &op,
            ctx,
            op.get_result(0).get_type(ctx),
            "comb.replicate result",
        )?;
        let count_attr = self
            .get_attr_count(ctx)
            .ok_or_else(|| -> Result<()> {
                verify_err!(op.loc(), "comb.replicate missing required attribute count",)
            })
            .unwrap();
        let count = count_attr.value().to_u64();
        if count == 0 {
            return verify_err!(op.loc(), "comb.replicate count must be positive, found 0");
        }
        let expected_w = (in_w as u64) * count;
        if res_w as u64 != expected_w {
            return verify_err!(
                op.loc(),
                "comb.replicate result width mismatch: expected {} (input width {} * count {}), found {}",
                expected_w,
                in_w,
                count,
                res_w
            );
        }
        Ok(())
    }
}

#[op_interface_impl]
impl SideEffects for ReplicateOp {
    fn has_side_effects(&self, _ctx: &Context) -> bool {
        false
    }
}

impl CombOpExt for ReplicateOp {
    fn width(&self, ctx: &Context) -> u32 {
        verify_integer_type(
            &self.get_operation().deref(ctx),
            ctx,
            self.result(ctx).get_type(ctx),
            "res",
        )
        .unwrap_or(0)
    }
}

impl ReplicateOp {
    /// Create a new `comb.replicate`.
    pub fn new(ctx: &mut Context, val: Value, count: IntegerAttr, res_ty: TypeHandle) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![res_ty],
            vec![val],
            vec![],
            0,
        );
        let rep = ReplicateOp { op };
        rep.set_attr_count(ctx, count);
        rep
    }

    /// Get result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }

    /// Get input value.
    pub fn val(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(0)
    }

    /// Get count attribute.
    pub fn count(&self, ctx: &Context) -> IntegerAttr {
        self.get_attr_count(ctx).expect("verified count").clone()
    }
}

/// Parity reduction operation: `comb.parity`
#[pliron_op(
    name = "comb.parity",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<1>],
)]
pub struct ParityOp;

impl Verify for ParityOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        verify_reduction(&op, ctx, "comb.parity")
    }
}

#[op_interface_impl]
impl SideEffects for ParityOp {
    fn has_side_effects(&self, _ctx: &Context) -> bool {
        false
    }
}

impl CombOpExt for ParityOp {
    fn width(&self, _ctx: &Context) -> u32 {
        1
    }
}

impl ParityOp {
    /// Create a new `comb.parity`.
    pub fn new(ctx: &mut Context, val: Value, i1_ty: TypeHandle) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![i1_ty],
            vec![val],
            vec![],
            0,
        );
        ParityOp { op }
    }

    /// Get result value (`i1`).
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }

    /// Get input value.
    pub fn val(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(0)
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
    NotOp::register(ctx);
    NegOp::register(ctx);
    AnyOp::register(ctx);
    AllOp::register(ctx);
    MuxOp::register(ctx);
    ICmpOp::register(ctx);
    ConcatOp::register(ctx);
    ExtractOp::register(ctx);
    ReplicateOp::register(ctx);
    ParityOp::register(ctx);
}

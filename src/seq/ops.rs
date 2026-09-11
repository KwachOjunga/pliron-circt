// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Operations defined in the `seq` dialect.

use pliron::{
    builtin::op_interfaces::{NOpdsInterface, NRegionsInterface, OneResultInterface},
    context::Context,
    derive::pliron_op,
    op::Op,
    operation::Operation,
    r#type::TypeHandle,
    value::Value,
};

/// A register sampled on the active edge of `clock`.
///
/// The operation has one cycle of state: its result is the current stored
/// value, and `input` becomes the stored value at the active clock edge.
#[pliron_op(
    name = "seq.compreg",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<2>],
    verifier = "succ",
)]
pub struct CompRegOp;

impl CompRegOp {
    /// Create a register with operands `(clock, input)` and result `result_ty`.
    pub fn new(ctx: &mut Context, clock: Value, input: Value, result_ty: TypeHandle) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![result_ty],
            vec![clock, input],
            vec![],
            0,
        );
        CompRegOp { op }
    }

    /// Get the register's current-value result.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// A glitch-safe clock gate.
///
/// The enable is sampled while the input clock is inactive; the result keeps
/// the input clock's identity while suppressing disabled active edges.
#[pliron_op(
    name = "seq.clock_gate",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<2>],
    verifier = "succ",
)]
pub struct ClockGateOp;

impl ClockGateOp {
    /// Create a gated clock from `(clock, enable)`.
    pub fn new(ctx: &mut Context, clock: Value, enable: Value, clock_ty: TypeHandle) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![clock_ty],
            vec![clock, enable],
            vec![],
            0,
        );
        ClockGateOp { op }
    }

    /// Get the gated clock result.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Register all `seq` operations in [Context].
pub fn register(ctx: &mut Context) {
    CompRegOp::register(ctx);
    ClockGateOp::register(ctx);
}
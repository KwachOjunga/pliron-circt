// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Operations defined in the `seq` dialect.

use pliron::{
    builtin::{
        attributes::{BoolAttr, StringAttr},
        op_interfaces::{NOpdsInterface, NRegionsInterface, OneResultInterface},
    },
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

/// A register with explicit reset behavior.
///
/// Operands are `(clock, input, reset, reset_value)`. At the active clock
/// edge, reset has priority over input; the reset value is loaded when reset
/// is asserted. The `is_async_reset` attribute distinguishes an immediate
/// reset from one sampled at the clock edge, and `reset_polarity` records the
/// external reset convention for lowering.
#[pliron_op(
    name = "seq.firreg",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<4>],
    attributes = (is_async_reset: BoolAttr, reset_polarity: StringAttr),
    verifier = "succ",
)]
pub struct FirRegOp;

impl FirRegOp {
    /// Create a resettable register with explicit reset policy attributes.
    pub fn new(
        ctx: &mut Context,
        clock: Value,
        input: Value,
        reset: Value,
        reset_value: Value,
        result_ty: TypeHandle,
        is_async_reset: bool,
        reset_polarity: impl Into<StringAttr>,
    ) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![result_ty],
            vec![clock, input, reset, reset_value],
            vec![],
            0,
        );
        let reg = FirRegOp { op };
        reg.set_attr_is_async_reset(ctx, is_async_reset.into());
        reg.set_attr_reset_polarity(ctx, reset_polarity.into());
        reg
    }

    /// Get the current register value.
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

/// Declares a high-level synchronous memory resource.
///
/// The result is a `!seq.mem<depth x element_type>` handle. Keeping the
/// handle abstract allows later lowering to choose an implementation without
/// changing the memory's addressability or stored value type.
#[pliron_op(
    name = "seq.hlmem",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<0>],
    attributes = (read_during_write: StringAttr),
    verifier = "succ",
)]
pub struct HLMemOp;

impl HLMemOp {
    /// Create a memory with a read-during-write policy of `read-first`,
    /// `write-first`, or `undefined`.
    pub fn new(
        ctx: &mut Context,
        memory_ty: TypeHandle,
        read_during_write: impl Into<StringAttr>,
    ) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![memory_ty],
            vec![],
            vec![],
            0,
        );
        let memory = HLMemOp { op };
        memory.set_attr_read_during_write(ctx, read_during_write.into());
        memory
    }

    /// Get the memory handle.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Synchronous one-cycle memory read.
///
/// Operands are `(clock, memory, address)`. The address is sampled on the
/// active edge and the result is the addressed word after one cycle.
#[pliron_op(
    name = "seq.hlmem_read",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<3>],
    verifier = "succ",
)]
pub struct HLMemReadOp;

impl HLMemReadOp {
    /// Create a synchronous read producing `element_ty`.
    pub fn new(
        ctx: &mut Context,
        clock: Value,
        memory: Value,
        address: Value,
        element_ty: TypeHandle,
    ) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![element_ty],
            vec![clock, memory, address],
            vec![],
            0,
        );
        HLMemReadOp { op }
    }

    /// Get the registered read data result.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Synchronous memory write.
///
/// Operands are `(clock, memory, address, data, enable)`. A write commits on
/// the active edge only when `enable` is asserted.
#[pliron_op(
    name = "seq.hlmem_write",
    format,
    interfaces = [NRegionsInterface<0>, NOpdsInterface<5>],
    verifier = "succ",
)]
pub struct HLMemWriteOp;

impl HLMemWriteOp {
    /// Create an enabled synchronous write.
    pub fn new(
        ctx: &mut Context,
        clock: Value,
        memory: Value,
        address: Value,
        data: Value,
        enable: Value,
    ) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![],
            vec![clock, memory, address, data, enable],
            vec![],
            0,
        );
        HLMemWriteOp { op }
    }
}

/// Register all `seq` operations in [Context].
pub fn register(ctx: &mut Context) {
    CompRegOp::register(ctx);
    FirRegOp::register(ctx);
    ClockGateOp::register(ctx);
    HLMemOp::register(ctx);
    HLMemReadOp::register(ctx);
    HLMemWriteOp::register(ctx);
}

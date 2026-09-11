// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Operations defined in the `seq` dialect.

use pliron::{
    builtin::{
        attributes::{BoolAttr, StringAttr},
        op_interfaces::{NOpdsInterface, NRegionsInterface, OneResultInterface},
    },
    common_traits::Verify,
    context::Context,
    derive::pliron_op,
    location::Located,
    op::Op,
    operation::Operation,
    result::Result,
    r#type::{TypeHandle, Typed},
    value::Value,
    verify_err,
};

use super::types::{ClockType, MemoryType, ResetType};

fn verify_clock(op: &Operation, ctx: &Context, index: usize) -> Result<()> {
    let expected: TypeHandle = ClockType::get(ctx).into();
    if op.get_operand(index).get_type(ctx) != expected {
        return verify_err!(op.loc(), "seq operand {} must have !seq.clock type", index);
    }
    Ok(())
}

fn verify_reset(op: &Operation, ctx: &Context, index: usize) -> Result<()> {
    let expected: TypeHandle = ResetType::get(ctx).into();
    if op.get_operand(index).get_type(ctx) != expected {
        return verify_err!(op.loc(), "seq operand {} must have !seq.reset type", index);
    }
    Ok(())
}

fn verify_i1(op: &Operation, ctx: &Context, index: usize) -> Result<()> {
    let expected = pliron::builtin::types::IntegerType::get(
        ctx,
        1,
        pliron::builtin::types::Signedness::Signless,
    );
    if op.get_operand(index).get_type(ctx) != expected.into() {
        return verify_err!(op.loc(), "seq operand {} must have i1 type", index);
    }
    Ok(())
}

fn verify_same_type(op: &Operation, ctx: &Context, lhs: usize, rhs: usize) -> Result<()> {
    if op.get_operand(lhs).get_type(ctx) != op.get_operand(rhs).get_type(ctx) {
        return verify_err!(
            op.loc(),
            "seq operands {} and {} must have the same type",
            lhs,
            rhs
        );
    }
    Ok(())
}

/// A register sampled on the active edge of `clock`.
///
/// The operation has one cycle of state: its result is the current stored
/// value, and `input` becomes the stored value at the active clock edge.
#[pliron_op(
    name = "seq.compreg",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<2>],
)]
pub struct CompRegOp;

impl Verify for CompRegOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        verify_clock(&op, ctx, 0)?;
        if op.get_operand(1).get_type(ctx) != op.get_result(0).get_type(ctx) {
            return verify_err!(
                op.loc(),
                "seq.compreg input and result must have the same type"
            );
        }
        Ok(())
    }
}

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

    /// Get the clock input.
    pub fn clock(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(0)
    }

    /// Get the next-state input.
    pub fn input(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(1)
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
)]
pub struct FirRegOp;

impl Verify for FirRegOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        verify_clock(&op, ctx, 0)?;
        verify_reset(&op, ctx, 2)?;
        verify_same_type(&op, ctx, 1, 3)?;
        if op.get_operand(1).get_type(ctx) != op.get_result(0).get_type(ctx) {
            return verify_err!(
                op.loc(),
                "seq.firreg input and result must have the same type"
            );
        }
        let polarity_attr = self
            .get_attr_reset_polarity(ctx)
            .expect("seq.firreg requires reset_polarity");
        let polarity = polarity_attr.as_ref();
        if polarity != "active_high" && polarity != "active_low" {
            return verify_err!(
                op.loc(),
                "seq.firreg reset_polarity must be active_high or active_low"
            );
        }
        Ok(())
    }
}

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

    /// Get the register clock.
    pub fn clock(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(0)
    }

    /// Get the next-state input.
    pub fn input(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(1)
    }

    /// Get the reset signal.
    pub fn reset(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(2)
    }

    /// Get the value loaded by reset.
    pub fn reset_value(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(3)
    }

    /// Whether reset is asynchronous.
    pub fn is_async_reset(&self, ctx: &Context) -> bool {
        self.get_attr_is_async_reset(ctx)
            .expect("seq.firreg requires is_async_reset")
            .clone()
            .into()
    }

    /// Get the reset polarity attribute.
    pub fn reset_polarity(&self, ctx: &Context) -> StringAttr {
        self.get_attr_reset_polarity(ctx)
            .expect("seq.firreg requires reset_polarity")
            .clone()
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
)]
pub struct ClockGateOp;

impl Verify for ClockGateOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        verify_clock(&op, ctx, 0)?;
        verify_i1(&op, ctx, 1)?;
        if op.get_result(0).get_type(ctx) != op.get_operand(0).get_type(ctx) {
            return verify_err!(op.loc(), "seq.clock_gate result must have !seq.clock type");
        }
        Ok(())
    }
}

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
)]
pub struct HLMemOp;

impl Verify for HLMemOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let memory = op
            .get_result(0)
            .get_type(ctx)
            .deref(ctx)
            .downcast_ref::<MemoryType>()
            .is_some();
        if !memory {
            return verify_err!(op.loc(), "seq.hlmem result must have !seq.mem type");
        }
        let policy_attr = self
            .get_attr_read_during_write(ctx)
            .expect("seq.hlmem requires read_during_write");
        let policy = policy_attr.as_ref();
        if policy != "read-first" && policy != "write-first" && policy != "undefined" {
            return verify_err!(
                op.loc(),
                "seq.hlmem read_during_write must be read-first, write-first, or undefined"
            );
        }
        Ok(())
    }
}

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
)]
pub struct HLMemReadOp;

impl Verify for HLMemReadOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        verify_clock(&op, ctx, 0)?;
        if op
            .get_operand(1)
            .get_type(ctx)
            .deref(ctx)
            .downcast_ref::<MemoryType>()
            .is_none()
        {
            return verify_err!(
                op.loc(),
                "seq.hlmem_read memory operand must have !seq.mem type"
            );
        }
        let memory_type = op.get_operand(1).get_type(ctx);
        let memory_type_ref = memory_type.deref(ctx);
        let memory = memory_type_ref.downcast_ref::<MemoryType>().unwrap();
        if memory.element_type() != op.get_result(0).get_type(ctx) {
            return verify_err!(
                op.loc(),
                "seq.hlmem_read result must match the memory element type"
            );
        }
        Ok(())
    }
}

impl HLMemReadOp {
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
)]
pub struct HLMemWriteOp;

impl Verify for HLMemWriteOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        verify_clock(&op, ctx, 0)?;
        verify_i1(&op, ctx, 4)?;
        let memory_type = op.get_operand(1).get_type(ctx);
        let memory_type_ref = memory_type.deref(ctx);
        let memory = match memory_type_ref.downcast_ref::<MemoryType>() {
            Some(memory) => memory,
            None => {
                return verify_err!(
                    op.loc(),
                    "seq.hlmem_write memory operand must have !seq.mem type"
                );
            }
        };
        if memory.element_type() != op.get_operand(3).get_type(ctx) {
            return verify_err!(
                op.loc(),
                "seq.hlmem_write data must match the memory element type"
            );
        }
        Ok(())
    }
}

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

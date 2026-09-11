// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Operations defined in the `sv` emission dialect.

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
    r#type::Typed,
    value::Value,
    verify_err,
};

use crate::seq::types::{ClockType, ResetType};

/// A named SystemVerilog continuous assignment.
///
/// The operand is the right-hand-side value and the result is the named
/// assignment value. The target name is retained for deterministic emission.
#[pliron_op(
    name = "sv.assign",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<1>],
    attributes = (assign_target: StringAttr),
)]
pub struct AssignOp;

impl Verify for AssignOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let target = self
            .get_attr_assign_target(ctx)
            .expect("sv.assign requires target")
            .clone();
        if target.as_ref().is_empty() {
            return verify_err!(op.loc(), "sv.assign target must not be empty");
        }
        if op.get_operand(0).get_type(ctx) != op.get_result(0).get_type(ctx) {
            return verify_err!(op.loc(), "sv.assign operand and result types must match");
        }
        Ok(())
    }
}

impl AssignOp {
    /// Create `assign target = value` with a result carrying the value type.
    pub fn new(ctx: &mut Context, target: impl Into<StringAttr>, value: Value) -> Self {
        let ty = value.get_type(ctx);
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![ty],
            vec![value],
            vec![],
            0,
        );
        let assign = AssignOp { op };
        assign.set_attr_assign_target(ctx, target.into());
        assign
    }

    /// Get the emitted target name.
    pub fn target(&self, ctx: &Context) -> StringAttr {
        self.get_attr_assign_target(ctx)
            .expect("sv.assign requires assign_target")
            .clone()
    }

    /// Get the assigned result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// A reset-aware SystemVerilog `always_ff` assignment.
///
/// Operands are `(clock, input, reset, reset_value)`. The operation emits a
/// nonblocking assignment and preserves synchronous/asynchronous reset intent
/// through its attributes.
#[pliron_op(
    name = "sv.always_ff",
    format,
    interfaces = [NRegionsInterface<0>, NOpdsInterface<4>],
    attributes = (ff_target: StringAttr, ff_async_reset: BoolAttr, ff_reset_polarity: StringAttr),
)]
pub struct AlwaysFfOp;

impl Verify for AlwaysFfOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let clock_ty: pliron::r#type::TypeHandle = ClockType::get(ctx).into();
        let reset_ty: pliron::r#type::TypeHandle = ResetType::get(ctx).into();
        if op.get_operand(0).get_type(ctx) != clock_ty {
            return verify_err!(op.loc(), "sv.always_ff clock must have !seq.clock type");
        }
        if op.get_operand(2).get_type(ctx) != reset_ty {
            return verify_err!(op.loc(), "sv.always_ff reset must have !seq.reset type");
        }
        if op.get_operand(1).get_type(ctx) != op.get_operand(3).get_type(ctx) {
            return verify_err!(
                op.loc(),
                "sv.always_ff input and reset value types must match"
            );
        }
        let target = self
            .get_attr_ff_target(ctx)
            .expect("sv.always_ff requires ff_target");
        if target.as_ref().is_empty() {
            return verify_err!(op.loc(), "sv.always_ff target must not be empty");
        }
        let polarity = self
            .get_attr_ff_reset_polarity(ctx)
            .expect("sv.always_ff requires ff_reset_polarity");
        if polarity.as_ref() != "active_high" && polarity.as_ref() != "active_low" {
            return verify_err!(
                op.loc(),
                "sv.always_ff reset_polarity must be active_high or active_low"
            );
        }
        Ok(())
    }
}

impl AlwaysFfOp {
    /// Create a reset-aware `always_ff` assignment for a named target.
    pub fn new(
        ctx: &mut Context,
        target: impl Into<StringAttr>,
        clock: Value,
        input: Value,
        reset: Value,
        reset_value: Value,
        is_async_reset: bool,
        reset_polarity: impl Into<StringAttr>,
    ) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![],
            vec![clock, input, reset, reset_value],
            vec![],
            0,
        );
        let always = AlwaysFfOp { op };
        always.set_attr_ff_target(ctx, target.into());
        always.set_attr_ff_async_reset(ctx, is_async_reset.into());
        always.set_attr_ff_reset_polarity(ctx, reset_polarity.into());
        always
    }
}

/// Register all `sv` operations in [Context].
pub fn register(ctx: &mut Context) {
    AssignOp::register(ctx);
    AlwaysFfOp::register(ctx);
}

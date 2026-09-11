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

/// A named SystemVerilog `logic` declaration.
#[pliron_op(
    name = "sv.logic_decl",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<0>],
    attributes = (logic_target: StringAttr),
)]
pub struct LogicDeclOp;

impl Verify for LogicDeclOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        if self
            .get_attr_logic_target(ctx)
            .expect("sv.logic_decl requires logic_target")
            .as_ref()
            .is_empty()
        {
            return verify_err!(op.loc(), "sv.logic_decl target must not be empty");
        }
        Ok(())
    }
}

impl LogicDeclOp {
    /// Declare a named `logic` value of `result_ty`.
    pub fn new(
        ctx: &mut Context,
        target: impl Into<StringAttr>,
        result_ty: pliron::r#type::TypeHandle,
    ) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![result_ty],
            vec![],
            vec![],
            0,
        );
        let declaration = LogicDeclOp { op };
        declaration.set_attr_logic_target(ctx, target.into());
        declaration
    }

    /// Get the declared value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// A named procedural combinational assignment.
#[pliron_op(
    name = "sv.always_comb",
    format,
    interfaces = [NRegionsInterface<0>, NOpdsInterface<1>],
    attributes = (comb_target: StringAttr),
)]
pub struct AlwaysCombOp;

impl Verify for AlwaysCombOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        if self
            .get_attr_comb_target(ctx)
            .expect("sv.always_comb requires comb_target")
            .as_ref()
            .is_empty()
        {
            return verify_err!(op.loc(), "sv.always_comb target must not be empty");
        }
        Ok(())
    }
}

impl AlwaysCombOp {
    /// Create `always_comb target = value` intent.
    pub fn new(ctx: &mut Context, target: impl Into<StringAttr>, value: Value) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![],
            vec![value],
            vec![],
            0,
        );
        let combinational = AlwaysCombOp { op };
        combinational.set_attr_comb_target(ctx, target.into());
        combinational
    }
}

/// A SystemVerilog module instance declaration.
#[pliron_op(
    name = "sv.instance",
    format,
    interfaces = [NRegionsInterface<0>],
    attributes = (sv_instance_name: StringAttr, sv_instance_module: StringAttr),
)]
pub struct InstanceOp;

impl Verify for InstanceOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        for (key, message) in [
            (self.get_attr_sv_instance_name(ctx), "instance name"),
            (self.get_attr_sv_instance_module(ctx), "module name"),
        ] {
            if key
                .expect("sv.instance requires instance attributes")
                .as_ref()
                .is_empty()
            {
                return verify_err!(op.loc(), "sv.instance {} must not be empty", message);
            }
        }
        Ok(())
    }
}

impl InstanceOp {
    /// Create an instance with input connections in operand order.
    pub fn new(
        ctx: &mut Context,
        instance_name: impl Into<StringAttr>,
        module_name: impl Into<StringAttr>,
        inputs: Vec<Value>,
    ) -> Self {
        let op = Operation::new(ctx, Self::get_concrete_op_info(), vec![], inputs, vec![], 0);
        let instance = InstanceOp { op };
        instance.set_attr_sv_instance_name(ctx, instance_name.into());
        instance.set_attr_sv_instance_module(ctx, module_name.into());
        instance
    }
}

/// A memory declaration retained for SystemVerilog emission.
#[pliron_op(
    name = "sv.mem_decl",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<0>],
    attributes = (memory_target: StringAttr),
)]
pub struct MemDeclOp;

impl Verify for MemDeclOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        if self
            .get_attr_memory_target(ctx)
            .expect("sv.mem_decl requires memory_target")
            .as_ref()
            .is_empty()
        {
            return verify_err!(op.loc(), "sv.mem_decl target must not be empty");
        }
        if op
            .get_result(0)
            .get_type(ctx)
            .deref(ctx)
            .downcast_ref::<crate::seq::types::MemoryType>()
            .is_none()
        {
            return verify_err!(op.loc(), "sv.mem_decl result must have !seq.mem type");
        }
        Ok(())
    }
}

impl MemDeclOp {
    /// Declare a `!seq.mem` resource for SV memory emission.
    pub fn new(
        ctx: &mut Context,
        target: impl Into<StringAttr>,
        memory_ty: pliron::r#type::TypeHandle,
    ) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![memory_ty],
            vec![],
            vec![],
            0,
        );
        let memory = MemDeclOp { op };
        memory.set_attr_memory_target(ctx, target.into());
        memory
    }
}

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

/// A reset-free SystemVerilog `always_ff` assignment for a plain D register.
///
/// This operation is the emission form for `seq.compreg`; it deliberately has
/// no reset operands or reset attributes so lowering cannot invent reset
/// behavior that was absent from the source IR.
#[pliron_op(
    name = "sv.always_ff_no_reset",
    format,
    interfaces = [NRegionsInterface<0>, NOpdsInterface<2>],
    attributes = (ff_nr_target: StringAttr),
)]
pub struct AlwaysFfNoResetOp;

impl Verify for AlwaysFfNoResetOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let clock_ty: pliron::r#type::TypeHandle = ClockType::get(ctx).into();
        if op.get_operand(0).get_type(ctx) != clock_ty {
            return verify_err!(
                op.loc(),
                "sv.always_ff_no_reset clock must have !seq.clock type"
            );
        }
        if self
            .get_attr_ff_nr_target(ctx)
            .expect("sv.always_ff_no_reset requires ff_nr_target")
            .as_ref()
            .is_empty()
        {
            return verify_err!(op.loc(), "sv.always_ff_no_reset target must not be empty");
        }
        Ok(())
    }
}

impl AlwaysFfNoResetOp {
    /// Create a reset-free `always_ff` assignment for a named target.
    pub fn new(
        ctx: &mut Context,
        target: impl Into<StringAttr>,
        clock: Value,
        input: Value,
    ) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![],
            vec![clock, input],
            vec![],
            0,
        );
        let always = AlwaysFfNoResetOp { op };
        always.set_attr_ff_nr_target(ctx, target.into());
        always
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
    LogicDeclOp::register(ctx);
    AlwaysCombOp::register(ctx);
    InstanceOp::register(ctx);
    MemDeclOp::register(ctx);
    AssignOp::register(ctx);
    AlwaysFfOp::register(ctx);
    AlwaysFfNoResetOp::register(ctx);
}

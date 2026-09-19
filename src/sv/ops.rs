// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Operations defined in the `sv` emission dialect.

use pliron::{
    builtin::{
        attributes::{BoolAttr, IntegerAttr, StringAttr},
        op_interfaces::{NOpdsInterface, NRegionsInterface, OneResultInterface},
        types::IntegerType,
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

/// A synchronous SystemVerilog memory read process.
#[pliron_op(
    name = "sv.mem_read",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<3>],
    attributes = (memory_read_target: StringAttr),
)]
pub struct MemReadOp;

impl Verify for MemReadOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        if op.get_operand(0).get_type(ctx) != ClockType::get(ctx).into() {
            return verify_err!(op.loc(), "sv.mem_read clock must have !seq.clock type");
        }
        let memory = op.get_operand(1).get_type(ctx);
        let memory_ref = memory.deref(ctx);
        let memory = match memory_ref.downcast_ref::<crate::seq::types::MemoryType>() {
            Some(memory) => memory,
            None => return verify_err!(op.loc(), "sv.mem_read memory must have !seq.mem type"),
        };
        if memory.element_type() != op.get_result(0).get_type(ctx) {
            return verify_err!(
                op.loc(),
                "sv.mem_read result must match memory element type"
            );
        }
        if self
            .get_attr_memory_read_target(ctx)
            .expect("sv.mem_read requires memory_read_target")
            .as_ref()
            .is_empty()
        {
            return verify_err!(op.loc(), "sv.mem_read target must not be empty");
        }
        Ok(())
    }
}

impl MemReadOp {
    /// Create a one-cycle synchronous read process for a named result.
    pub fn new(
        ctx: &mut Context,
        target: impl Into<StringAttr>,
        clock: Value,
        memory: Value,
        address: Value,
        result_ty: pliron::r#type::TypeHandle,
    ) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![result_ty],
            vec![clock, memory, address],
            vec![],
            0,
        );
        let read = MemReadOp { op };
        read.set_attr_memory_read_target(ctx, target.into());
        read
    }

    /// Get the registered read result.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// A synchronous SystemVerilog memory write process.
#[pliron_op(
    name = "sv.mem_write",
    format,
    interfaces = [NRegionsInterface<0>, NOpdsInterface<5>],
    attributes = (memory_write_target: StringAttr),
)]
pub struct MemWriteOp;

impl Verify for MemWriteOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        if op.get_operand(0).get_type(ctx) != ClockType::get(ctx).into() {
            return verify_err!(op.loc(), "sv.mem_write clock must have !seq.clock type");
        }
        if op.get_operand(4).get_type(ctx)
            != pliron::builtin::types::IntegerType::get(
                ctx,
                1,
                pliron::builtin::types::Signedness::Signless,
            )
            .into()
        {
            return verify_err!(op.loc(), "sv.mem_write enable must have i1 type");
        }
        let memory = op.get_operand(1).get_type(ctx);
        let memory_ref = memory.deref(ctx);
        let memory = match memory_ref.downcast_ref::<crate::seq::types::MemoryType>() {
            Some(memory) => memory,
            None => return verify_err!(op.loc(), "sv.mem_write memory must have !seq.mem type"),
        };
        if memory.element_type() != op.get_operand(3).get_type(ctx) {
            return verify_err!(op.loc(), "sv.mem_write data must match memory element type");
        }
        if self
            .get_attr_memory_write_target(ctx)
            .expect("sv.mem_write requires memory_write_target")
            .as_ref()
            .is_empty()
        {
            return verify_err!(op.loc(), "sv.mem_write target must not be empty");
        }
        Ok(())
    }
}

impl MemWriteOp {
    /// Create an enabled synchronous write process for a named memory.
    pub fn new(
        ctx: &mut Context,
        target: impl Into<StringAttr>,
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
        let write = MemWriteOp { op };
        write.set_attr_memory_write_target(ctx, target.into());
        write
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

    /// Get the declared memory resource.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
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
    #[allow(clippy::too_many_arguments)]
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

/// A named SystemVerilog `wire` net declaration.
#[pliron_op(
    name = "sv.wire_decl",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<0>],
    attributes = (wire_target: StringAttr),
)]
pub struct WireDeclOp;

impl Verify for WireDeclOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let target = self
            .get_attr_wire_target(ctx)
            .expect("sv.wire_decl requires wire_target");
        if target.as_ref().is_empty() {
            return verify_err!(op.loc(), "sv.wire_decl target must not be empty");
        }
        Ok(())
    }
}

impl WireDeclOp {
    /// Declare a named `wire` net of `result_ty`.
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
        let wire = WireDeclOp { op };
        wire.set_attr_wire_target(ctx, target.into());
        wire
    }

    /// Get the declared value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }

    /// Get the net target name.
    pub fn target(&self, ctx: &Context) -> StringAttr {
        self.get_attr_wire_target(ctx)
            .expect("verified wire target")
            .clone()
    }
}

/// A named SystemVerilog `reg` variable declaration.
#[pliron_op(
    name = "sv.reg_decl",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<0>],
    attributes = (reg_target: StringAttr),
)]
pub struct RegDeclOp;

impl Verify for RegDeclOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let target = self
            .get_attr_reg_target(ctx)
            .expect("sv.reg_decl requires reg_target");
        if target.as_ref().is_empty() {
            return verify_err!(op.loc(), "sv.reg_decl target must not be empty");
        }
        Ok(())
    }
}

impl RegDeclOp {
    /// Declare a named `reg` variable of `result_ty`.
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
        let reg = RegDeclOp { op };
        reg.set_attr_reg_target(ctx, target.into());
        reg
    }

    /// Get the declared value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }

    /// Get the reg target name.
    pub fn target(&self, ctx: &Context) -> StringAttr {
        self.get_attr_reg_target(ctx)
            .expect("verified reg target")
            .clone()
    }
}

/// A SystemVerilog binary expression (e.g. `+`, `-`, `*`, `/`, `%`, `&`, `|`, `^`, `<<`, `>>`, `>>>`, `==`, `!=`, `<`, `<=`, `>`, `>=`).
#[pliron_op(
    name = "sv.binary_expr",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<2>],
    attributes = (binary_operator: StringAttr),
)]
pub struct BinaryExprOp;

impl Verify for BinaryExprOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let op_str = self
            .get_attr_binary_operator(ctx)
            .expect("sv.binary_expr requires binary_operator");
        let valid_ops = [
            "+", "-", "*", "/", "%", "<<", ">>", ">>>", "&", "|", "^", "~^", "^~", "==", "!=", "<",
            "<=", ">", ">=", "&&", "||",
        ];
        if !valid_ops.contains(&op_str.as_ref()) {
            return verify_err!(
                op.loc(),
                "sv.binary_expr unrecognized operator '{}'",
                op_str.as_ref()
            );
        }
        Ok(())
    }
}

impl BinaryExprOp {
    /// Create a new SystemVerilog binary expression.
    pub fn new(
        ctx: &mut Context,
        operator: impl Into<StringAttr>,
        lhs: Value,
        rhs: Value,
        res_ty: pliron::r#type::TypeHandle,
    ) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![res_ty],
            vec![lhs, rhs],
            vec![],
            0,
        );
        let expr = BinaryExprOp { op };
        expr.set_attr_binary_operator(ctx, operator.into());
        expr
    }

    /// Get the expression result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }

    /// Get the operator string.
    pub fn operator(&self, ctx: &Context) -> StringAttr {
        self.get_attr_binary_operator(ctx)
            .expect("verified binary operator")
            .clone()
    }

    /// Get the left-hand-side operand.
    pub fn lhs(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(0)
    }

    /// Get the right-hand-side operand.
    pub fn rhs(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(1)
    }
}

/// A SystemVerilog unary expression (e.g. `~`, `!`, `-`, `&`, `|`, `^`).
#[pliron_op(
    name = "sv.unary_expr",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<1>],
    attributes = (unary_operator: StringAttr),
)]
pub struct UnaryExprOp;

impl Verify for UnaryExprOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let op_str = self
            .get_attr_unary_operator(ctx)
            .expect("sv.unary_expr requires unary_operator");
        let valid_ops = ["~", "!", "-", "&", "|", "^", "~&", "~|", "~^", "^~"];
        if !valid_ops.contains(&op_str.as_ref()) {
            return verify_err!(
                op.loc(),
                "sv.unary_expr unrecognized operator '{}'",
                op_str.as_ref()
            );
        }
        Ok(())
    }
}

impl UnaryExprOp {
    /// Create a new SystemVerilog unary expression.
    pub fn new(
        ctx: &mut Context,
        operator: impl Into<StringAttr>,
        val: Value,
        res_ty: pliron::r#type::TypeHandle,
    ) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![res_ty],
            vec![val],
            vec![],
            0,
        );
        let expr = UnaryExprOp { op };
        expr.set_attr_unary_operator(ctx, operator.into());
        expr
    }

    /// Get the expression result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }

    /// Get the operator string.
    pub fn operator(&self, ctx: &Context) -> StringAttr {
        self.get_attr_unary_operator(ctx)
            .expect("verified unary operator")
            .clone()
    }

    /// Get the source operand value.
    pub fn val(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(0)
    }
}

/// A SystemVerilog ternary multiplexer expression: `cond ? true_val : false_val`.
#[pliron_op(
    name = "sv.mux_expr",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<3>],
)]
pub struct MuxExprOp;

impl Verify for MuxExprOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let cond_ty = op.get_operand(0).get_type(ctx);
        let i1_ty = pliron::builtin::types::IntegerType::get(
            ctx,
            1,
            pliron::builtin::types::Signedness::Signless,
        );
        if cond_ty != i1_ty.into() {
            return verify_err!(op.loc(), "sv.mux_expr condition must be i1");
        }
        let true_ty = op.get_operand(1).get_type(ctx);
        let false_ty = op.get_operand(2).get_type(ctx);
        let res_ty = op.get_result(0).get_type(ctx);
        if true_ty != false_ty || true_ty != res_ty {
            return verify_err!(
                op.loc(),
                "sv.mux_expr true_val, false_val, and result types must match"
            );
        }
        Ok(())
    }
}

impl MuxExprOp {
    /// Create a new SystemVerilog ternary multiplexer expression.
    pub fn new(
        ctx: &mut Context,
        cond: Value,
        true_val: Value,
        false_val: Value,
        res_ty: pliron::r#type::TypeHandle,
    ) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![res_ty],
            vec![cond, true_val, false_val],
            vec![],
            0,
        );
        MuxExprOp { op }
    }

    /// Get the expression result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }

    /// Get the condition operand.
    pub fn cond(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(0)
    }

    /// Get the true-branch value.
    pub fn true_val(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(1)
    }

    /// Get the false-branch value.
    pub fn false_val(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(2)
    }
}

/// A SystemVerilog concatenation expression: `{a, b, c}`.
#[pliron_op(
    name = "sv.concat_expr",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface],
)]
pub struct ConcatExprOp;

impl Verify for ConcatExprOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        if op.get_num_operands() == 0 {
            return verify_err!(op.loc(), "sv.concat_expr requires at least one operand");
        }
        Ok(())
    }
}

impl ConcatExprOp {
    /// Create a new SystemVerilog concatenation expression.
    pub fn new(ctx: &mut Context, inputs: Vec<Value>, res_ty: pliron::r#type::TypeHandle) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![res_ty],
            inputs,
            vec![],
            0,
        );
        ConcatExprOp { op }
    }

    /// Get the expression result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }

    /// Get the concatenated inputs.
    pub fn inputs(&self, ctx: &Context) -> Vec<Value> {
        let op = self.get_operation().deref(ctx);
        (0..op.get_num_operands())
            .map(|i| op.get_operand(i))
            .collect()
    }
}

/// A SystemVerilog bit slice expression: `val[msb:lsb]`.
#[pliron_op(
    name = "sv.slice_expr",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<1>],
    attributes = (slice_low_bit: IntegerAttr, slice_width: IntegerAttr),
)]
pub struct SliceExprOp;

impl Verify for SliceExprOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let width_attr = self
            .get_attr_slice_width(ctx)
            .expect("sv.slice_expr requires slice_width");
        let width_val: u64 = width_attr.value().to_u64();
        if width_val == 0 {
            return verify_err!(op.loc(), "sv.slice_expr width must be positive");
        }
        let res_ty = op.get_result(0).get_type(ctx);
        if let Some(int_ty) = res_ty.deref(ctx).downcast_ref::<IntegerType>()
            && int_ty.width() as u64 != width_val
        {
            return verify_err!(
                op.loc(),
                "sv.slice_expr result width {} does not match attribute {}",
                int_ty.width(),
                width_val
            );
        }

        Ok(())
    }
}

impl SliceExprOp {
    /// Create a new SystemVerilog bit slice expression.
    pub fn new(
        ctx: &mut Context,
        val: Value,
        low_bit: impl Into<IntegerAttr>,
        width: impl Into<IntegerAttr>,
        res_ty: pliron::r#type::TypeHandle,
    ) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![res_ty],
            vec![val],
            vec![],
            0,
        );
        let slice = SliceExprOp { op };
        slice.set_attr_slice_low_bit(ctx, low_bit.into());
        slice.set_attr_slice_width(ctx, width.into());
        slice
    }

    /// Get the slice result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }

    /// Get the sliced operand.
    pub fn val(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(0)
    }

    /// Get the low bit index.
    pub fn low_bit(&self, ctx: &Context) -> IntegerAttr {
        self.get_attr_slice_low_bit(ctx)
            .expect("verified slice_low_bit")
            .clone()
    }

    /// Get the slice width.
    pub fn width(&self, ctx: &Context) -> IntegerAttr {
        self.get_attr_slice_width(ctx)
            .expect("verified slice_width")
            .clone()
    }
}

/// A SystemVerilog array or vector index expression: `val[index]`.
#[pliron_op(
    name = "sv.index_expr",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<2>],
)]
pub struct IndexExprOp;

impl Verify for IndexExprOp {
    fn verify(&self, _ctx: &Context) -> Result<()> {
        Ok(())
    }
}

impl IndexExprOp {
    /// Create a new SystemVerilog index expression.
    pub fn new(
        ctx: &mut Context,
        val: Value,
        index: Value,
        res_ty: pliron::r#type::TypeHandle,
    ) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![res_ty],
            vec![val, index],
            vec![],
            0,
        );
        IndexExprOp { op }
    }

    /// Get the index result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }

    /// Get the indexed value.
    pub fn val(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(0)
    }

    /// Get the index expression value.
    pub fn index(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(1)
    }
}

/// A SystemVerilog literal constant expression.
#[pliron_op(
    name = "sv.constant",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<0>],
    attributes = (constant_value: IntegerAttr, constant_width: IntegerAttr),
)]
pub struct ConstantExprOp;

impl Verify for ConstantExprOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let width_attr = self
            .get_attr_constant_width(ctx)
            .expect("sv.constant requires constant_width");
        let width_val: u64 = width_attr.value().to_u64();
        if width_val == 0 {
            return verify_err!(op.loc(), "sv.constant width must be positive");
        }
        let res_ty = op.get_result(0).get_type(ctx);
        if let Some(int_ty) = res_ty.deref(ctx).downcast_ref::<IntegerType>()
            && int_ty.width() as u64 != width_val
        {
            return verify_err!(
                op.loc(),
                "sv.constant result width {} does not match attribute {}",
                int_ty.width(),
                width_val
            );
        }

        Ok(())
    }
}

impl ConstantExprOp {
    /// Create a new SystemVerilog constant expression.
    pub fn new(
        ctx: &mut Context,
        value: impl Into<IntegerAttr>,
        width: impl Into<IntegerAttr>,
        res_ty: pliron::r#type::TypeHandle,
    ) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![res_ty],
            vec![],
            vec![],
            0,
        );
        let constant = ConstantExprOp { op };
        constant.set_attr_constant_value(ctx, value.into());
        constant.set_attr_constant_width(ctx, width.into());
        constant
    }

    /// Get the constant result value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }

    /// Get the constant value.
    pub fn value(&self, ctx: &Context) -> IntegerAttr {
        self.get_attr_constant_value(ctx)
            .expect("verified value")
            .clone()
    }

    /// Get the constant bitwidth.
    pub fn width(&self, ctx: &Context) -> IntegerAttr {
        self.get_attr_constant_width(ctx)
            .expect("verified width")
            .clone()
    }
}

/// A SystemVerilog blocking procedural assignment: `target = value;`.
#[pliron_op(
    name = "sv.bpa",
    format,
    interfaces = [NRegionsInterface<0>, NOpdsInterface<1>],
    attributes = (bpa_target: StringAttr),
)]
pub struct BpaOp;

impl Verify for BpaOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let target = self
            .get_attr_bpa_target(ctx)
            .expect("sv.bpa requires bpa_target");
        if target.as_ref().is_empty() {
            return verify_err!(op.loc(), "sv.bpa target must not be empty");
        }
        Ok(())
    }
}

impl BpaOp {
    /// Create a new blocking procedural assignment.
    pub fn new(ctx: &mut Context, target: impl Into<StringAttr>, value: Value) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![],
            vec![value],
            vec![],
            0,
        );
        let bpa = BpaOp { op };
        bpa.set_attr_bpa_target(ctx, target.into());
        bpa
    }

    /// Get the target name.
    pub fn target(&self, ctx: &Context) -> StringAttr {
        self.get_attr_bpa_target(ctx)
            .expect("verified bpa target")
            .clone()
    }

    /// Get the assigned value.
    pub fn value(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(0)
    }
}

/// A SystemVerilog non-blocking procedural assignment: `target <= value;`.
#[pliron_op(
    name = "sv.nba",
    format,
    interfaces = [NRegionsInterface<0>, NOpdsInterface<1>],
    attributes = (nba_target: StringAttr),
)]
pub struct NbaOp;

impl Verify for NbaOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let target = self
            .get_attr_nba_target(ctx)
            .expect("sv.nba requires nba_target");
        if target.as_ref().is_empty() {
            return verify_err!(op.loc(), "sv.nba target must not be empty");
        }
        Ok(())
    }
}

impl NbaOp {
    /// Create a new non-blocking procedural assignment.
    pub fn new(ctx: &mut Context, target: impl Into<StringAttr>, value: Value) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![],
            vec![value],
            vec![],
            0,
        );
        let nba = NbaOp { op };
        nba.set_attr_nba_target(ctx, target.into());
        nba
    }

    /// Get the target name.
    pub fn target(&self, ctx: &Context) -> StringAttr {
        self.get_attr_nba_target(ctx)
            .expect("verified nba target")
            .clone()
    }

    /// Get the assigned value.
    pub fn value(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(0)
    }
}

/// A SystemVerilog procedural `case` statement.
#[pliron_op(
    name = "sv.case",
    format,
    interfaces = [NRegionsInterface<0>, NOpdsInterface<1>],
    attributes = (case_target: StringAttr),
)]
pub struct CaseOp;

impl Verify for CaseOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let target = self
            .get_attr_case_target(ctx)
            .expect("sv.case requires case_target");
        if target.as_ref().is_empty() {
            return verify_err!(op.loc(), "sv.case target must not be empty");
        }
        Ok(())
    }
}

impl CaseOp {
    /// Create a new case statement header for a target variable.
    pub fn new(ctx: &mut Context, target: impl Into<StringAttr>, selector: Value) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![],
            vec![selector],
            vec![],
            0,
        );
        let case_op = CaseOp { op };
        case_op.set_attr_case_target(ctx, target.into());
        case_op
    }

    /// Get the target name.
    pub fn target(&self, ctx: &Context) -> StringAttr {
        self.get_attr_case_target(ctx)
            .expect("verified case target")
            .clone()
    }

    /// Get the selector value.
    pub fn selector(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(0)
    }
}

/// Register all `sv` operations in [Context].
pub fn register(ctx: &mut Context) {
    LogicDeclOp::register(ctx);
    WireDeclOp::register(ctx);
    RegDeclOp::register(ctx);
    AlwaysCombOp::register(ctx);
    InstanceOp::register(ctx);
    MemDeclOp::register(ctx);
    MemReadOp::register(ctx);
    MemWriteOp::register(ctx);
    AssignOp::register(ctx);
    AlwaysFfOp::register(ctx);
    AlwaysFfNoResetOp::register(ctx);
    BinaryExprOp::register(ctx);
    UnaryExprOp::register(ctx);
    MuxExprOp::register(ctx);
    ConcatExprOp::register(ctx);
    SliceExprOp::register(ctx);
    IndexExprOp::register(ctx);
    ConstantExprOp::register(ctx);
    BpaOp::register(ctx);
    NbaOp::register(ctx);
    CaseOp::register(ctx);
}

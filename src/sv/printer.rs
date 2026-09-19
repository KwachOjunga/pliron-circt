// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Deterministic SystemVerilog emission for the implemented `sv` operations.

use std::fmt::Write;

use pliron::{
    builtin::{op_interfaces::SymbolOpInterface, types::IntegerType},
    common_traits::Named,
    context::Context,
    linked_list::ContainsLinkedList,
    location::Located,
    op::{Op, verify_op},
    operation::Operation,
    printable::Printable,
    result::Result,
    r#type::Typed,
    value::Value,
    verify_err,
};

use crate::{
    hw::ops::ModuleOp,
    seq::{
        ops::{CompRegOp, FirRegOp, HLMemOp, HLMemReadOp, HLMemWriteOp},
        types::{ClockType, MemoryType, ResetType},
    },
    sv::ops::{
        AlwaysCombOp, AlwaysFfNoResetOp, AlwaysFfOp, AssignOp, BinaryExprOp, BpaOp, CaseOp,
        ConcatExprOp, ConstantExprOp, IndexExprOp, InstanceOp, LogicDeclOp, MemDeclOp, MemReadOp,
        MemWriteOp, MuxExprOp, NbaOp, RegDeclOp, SliceExprOp, UnaryExprOp, WireDeclOp,
    },
};

fn value_name(ctx: &Context, value: Value) -> String {
    value
        .given_name(ctx)
        .map(|name| name.to_string())
        .unwrap_or_else(|| value.unique_name(ctx).to_string())
}

fn render_expr(ctx: &Context, value: Value) -> String {
    if let Some(defining_op) = value.defining_op() {
        if let Some(bin) = Operation::get_op::<BinaryExprOp>(defining_op, ctx) {
            return format!(
                "{} {} {}",
                render_expr(ctx, bin.lhs(ctx)),
                bin.operator(ctx).as_ref(),
                render_expr(ctx, bin.rhs(ctx))
            );
        }
        if let Some(un) = Operation::get_op::<UnaryExprOp>(defining_op, ctx) {
            return format!(
                "{}{}",
                un.operator(ctx).as_ref(),
                render_expr(ctx, un.val(ctx))
            );
        }
        if let Some(constant) = Operation::get_op::<ConstantExprOp>(defining_op, ctx) {
            let val: u64 = constant.value(ctx).value().to_u64();
            let width: u64 = constant.width(ctx).value().to_u64();
            if width == 32 || width == 0 {
                return format!("{}", val);
            } else {
                return format!("{}'d{}", width, val);
            }
        }
        if let Some(mux) = Operation::get_op::<MuxExprOp>(defining_op, ctx) {
            return format!(
                "{} ? {} : {}",
                render_expr(ctx, mux.cond(ctx)),
                render_expr(ctx, mux.true_val(ctx)),
                render_expr(ctx, mux.false_val(ctx))
            );
        }
        if let Some(concat) = Operation::get_op::<ConcatExprOp>(defining_op, ctx) {
            let in_strs: Vec<String> = concat
                .inputs(ctx)
                .into_iter()
                .map(|v| render_expr(ctx, v))
                .collect();
            return format!("{{{}}}", in_strs.join(", "));
        }
        if let Some(slice) = Operation::get_op::<SliceExprOp>(defining_op, ctx) {
            let low: u64 = slice.low_bit(ctx).value().to_u64();
            let width: u64 = slice.width(ctx).value().to_u64();
            if width == 1 {
                return format!("{}[{}]", render_expr(ctx, slice.val(ctx)), low);
            } else {
                let high = low + width - 1;
                return format!("{}[{}:{}]", render_expr(ctx, slice.val(ctx)), high, low);
            }
        }
        if let Some(idx) = Operation::get_op::<IndexExprOp>(defining_op, ctx) {
            return format!(
                "{}[{}]",
                render_expr(ctx, idx.val(ctx)),
                render_expr(ctx, idx.index(ctx))
            );
        }
    }
    value_name(ctx, value)
}

fn sv_type_handle(ctx: &Context, ty: pliron::r#type::TypeHandle) -> String {
    let type_ref = ty.deref(ctx);
    if let Some(integer) = type_ref.downcast_ref::<IntegerType>() {
        if integer.width() == 1 {
            "logic".to_string()
        } else {
            format!("logic [{}:0]", integer.width() - 1)
        }
    } else if type_ref.downcast_ref::<ClockType>().is_some()
        || type_ref.downcast_ref::<ResetType>().is_some()
    {
        "logic".to_string()
    } else {
        ty.disp(ctx).to_string()
    }
}

fn sv_type(ctx: &Context, value: Value) -> String {
    sv_type_handle(ctx, value.get_type(ctx))
}

fn sv_decl_type(ctx: &Context, value: Value, kind: &str) -> String {
    let ty = value.get_type(ctx);
    if let Some(integer) = ty.deref(ctx).downcast_ref::<IntegerType>() {
        if integer.width() == 1 {
            kind.to_string()
        } else {
            format!("{} [{}:0]", kind, integer.width() - 1)
        }
    } else {
        format!("{} {}", kind, sv_type(ctx, value))
    }
}

fn memory_name(ctx: &Context, value: Value) -> String {
    if let Some(defining_op) = value.defining_op()
        && let Some(memory) = Operation::get_op::<MemDeclOp>(defining_op, ctx)
    {
        return memory
            .get_attr_memory_target(ctx)
            .expect("verified memory target")
            .as_ref()
            .to_owned();
    }
    value_name(ctx, value)
}

fn render_op(ctx: &Context, op_ptr: pliron::context::Ptr<Operation>) -> Option<String> {
    if let Some(comb) = Operation::get_op::<AlwaysCombOp>(op_ptr, ctx) {
        let op = comb.get_operation().deref(ctx);
        return Some(format!(
            "always_comb begin\n  {} = {};\nend",
            comb.get_attr_comb_target(ctx)
                .expect("verified always_comb target")
                .as_ref(),
            render_expr(ctx, op.get_operand(0))
        ));
    }
    if let Some(instance) = Operation::get_op::<InstanceOp>(op_ptr, ctx) {
        let op = instance.get_operation().deref(ctx);
        let inputs = (0..op.get_num_operands())
            .map(|index| render_expr(ctx, op.get_operand(index)))
            .collect::<Vec<_>>()
            .join(", ");
        return Some(format!(
            "{} {} ({});",
            instance
                .get_attr_sv_instance_module(ctx)
                .expect("verified instance module")
                .as_ref(),
            instance
                .get_attr_sv_instance_name(ctx)
                .expect("verified instance name")
                .as_ref(),
            inputs
        ));
    }
    if let Some(assign) = Operation::get_op::<AssignOp>(op_ptr, ctx) {
        let op = assign.get_operation().deref(ctx);
        let value = op.get_operand(0);
        return Some(format!(
            "assign {} = {};",
            assign.target(ctx).as_ref(),
            render_expr(ctx, value)
        ));
    }
    if let Some(bpa) = Operation::get_op::<BpaOp>(op_ptr, ctx) {
        return Some(format!(
            "{} = {};",
            bpa.target(ctx).as_ref(),
            render_expr(ctx, bpa.value(ctx))
        ));
    }
    if let Some(nba) = Operation::get_op::<NbaOp>(op_ptr, ctx) {
        return Some(format!(
            "{} <= {};",
            nba.target(ctx).as_ref(),
            render_expr(ctx, nba.value(ctx))
        ));
    }
    if let Some(declaration) = Operation::get_op::<LogicDeclOp>(op_ptr, ctx) {
        return Some(format!(
            "{} {};",
            sv_decl_type(ctx, declaration.result(ctx), "logic"),
            declaration
                .get_attr_logic_target(ctx)
                .expect("verified logic target")
                .as_ref()
        ));
    }
    if let Some(declaration) = Operation::get_op::<WireDeclOp>(op_ptr, ctx) {
        return Some(format!(
            "{} {};",
            sv_decl_type(ctx, declaration.result(ctx), "wire"),
            declaration.target(ctx).as_ref()
        ));
    }
    if let Some(declaration) = Operation::get_op::<RegDeclOp>(op_ptr, ctx) {
        return Some(format!(
            "{} {};",
            sv_decl_type(ctx, declaration.result(ctx), "reg"),
            declaration.target(ctx).as_ref()
        ));
    }
    if let Some(memory) = Operation::get_op::<MemDeclOp>(op_ptr, ctx) {
        let op = memory.get_operation().deref(ctx);
        let memory_type = op.get_result(0).get_type(ctx);
        let memory_type_ref = memory_type.deref(ctx);
        let memory_type = memory_type_ref
            .downcast_ref::<MemoryType>()
            .expect("verified memory declaration type");
        let element = sv_type_handle(ctx, memory_type.element_type());
        return Some(format!(
            "{} {} [0:{}];",
            element,
            memory
                .get_attr_memory_target(ctx)
                .expect("verified memory target")
                .as_ref(),
            memory_type.depth() - 1
        ));
    }
    if let Some(read) = Operation::get_op::<MemReadOp>(op_ptr, ctx) {
        let op = read.get_operation().deref(ctx);
        return Some(format!(
            "always_ff @(posedge {}) begin\n  {} <= {}[{}];\nend",
            value_name(ctx, op.get_operand(0)),
            read.get_attr_memory_read_target(ctx)
                .expect("verified read target")
                .as_ref(),
            memory_name(ctx, op.get_operand(1)),
            render_expr(ctx, op.get_operand(2))
        ));
    }
    if let Some(write) = Operation::get_op::<MemWriteOp>(op_ptr, ctx) {
        let op = write.get_operation().deref(ctx);
        return Some(format!(
            "always_ff @(posedge {}) begin\n  if ({})\n    {}[{}] <= {};\nend",
            value_name(ctx, op.get_operand(0)),
            render_expr(ctx, op.get_operand(4)),
            memory_name(ctx, op.get_operand(1)),
            render_expr(ctx, op.get_operand(2)),
            render_expr(ctx, op.get_operand(3))
        ));
    }
    if let Some(always) = Operation::get_op::<AlwaysFfNoResetOp>(op_ptr, ctx) {
        let op = always.get_operation().deref(ctx);
        return Some(format!(
            "always_ff @(posedge {}) begin\n  {} <= {};\nend",
            value_name(ctx, op.get_operand(0)),
            always
                .get_attr_ff_nr_target(ctx)
                .expect("verified always_ff_no_reset target")
                .as_ref(),
            render_expr(ctx, op.get_operand(1))
        ));
    }
    if let Some(always) = Operation::get_op::<AlwaysFfOp>(op_ptr, ctx) {
        let op = always.get_operation().deref(ctx);
        let polarity = always
            .get_attr_ff_reset_polarity(ctx)
            .expect("verified always_ff reset polarity");
        let async_reset = bool::from(
            always
                .get_attr_ff_async_reset(ctx)
                .expect("verified always_ff reset mode")
                .clone(),
        );
        let reset_name = value_name(ctx, op.get_operand(2));
        let reset_event = if polarity.as_ref() == "active_high" {
            format!("posedge {}", reset_name)
        } else {
            format!("negedge {}", reset_name)
        };
        let event = if async_reset {
            format!(
                "posedge {} or {}",
                value_name(ctx, op.get_operand(0)),
                reset_event
            )
        } else {
            format!("posedge {}", value_name(ctx, op.get_operand(0)))
        };
        let condition = if polarity.as_ref() == "active_high" {
            reset_name
        } else {
            format!("!{}", reset_name)
        };
        return Some(format!(
            "always_ff @({}) begin\n  if ({})\n    {} <= {};\n  else\n    {} <= {};\nend",
            event,
            condition,
            always
                .get_attr_ff_target(ctx)
                .expect("verified always_ff target")
                .as_ref(),
            render_expr(ctx, op.get_operand(3)),
            always
                .get_attr_ff_target(ctx)
                .expect("verified always_ff target")
                .as_ref(),
            render_expr(ctx, op.get_operand(1))
        ));
    }
    None
}

/// Render the implemented SV operations in a verified `hw.module`.
///
/// Operations outside the implemented SV emission surface are omitted rather
/// than guessed. Callers should lower those operations before printing.
pub fn render_module(ctx: &Context, module: &ModuleOp) -> Result<String> {
    verify_op(module, ctx)?;
    let name = module.get_symbol_name(ctx);
    let mut output = format!("module {} (\n", name);
    for index in 0..module.num_inputs(ctx) {
        let input = module.get_input(ctx, index);
        let input_name = value_name(ctx, input);
        let comma = if index + 1 == module.num_inputs(ctx) {
            ""
        } else {
            ","
        };
        writeln!(
            output,
            "  input {} {}{}",
            sv_type(ctx, input),
            input_name,
            comma
        )
        .unwrap();
    }
    output.push_str(");\n");
    let mut declarations = Vec::new();
    let mut declared_names = rustc_hash::FxHashSet::default();

    for index in 0..module.num_inputs(ctx) {
        let input = module.get_input(ctx, index);
        declared_names.insert(value_name(ctx, input));
    }

    let mut statements = Vec::new();
    for op_ptr in module.get_body(ctx).deref(ctx).iter(ctx) {
        if let Some(decl) = Operation::get_op::<LogicDeclOp>(op_ptr, ctx) {
            let target = decl
                .get_attr_logic_target(ctx)
                .expect("verified logic target")
                .as_ref()
                .to_string();
            if declared_names.insert(target.clone()) {
                declarations.push(format!(
                    "{} {};",
                    sv_decl_type(ctx, decl.result(ctx), "logic"),
                    target
                ));
            }
        } else if let Some(decl) = Operation::get_op::<WireDeclOp>(op_ptr, ctx) {
            let target = decl.target(ctx).as_ref().to_string();
            if declared_names.insert(target.clone()) {
                declarations.push(format!(
                    "{} {};",
                    sv_decl_type(ctx, decl.result(ctx), "wire"),
                    target
                ));
            }
        } else if let Some(decl) = Operation::get_op::<RegDeclOp>(op_ptr, ctx) {
            let target = decl.target(ctx).as_ref().to_string();
            if declared_names.insert(target.clone()) {
                declarations.push(format!(
                    "{} {};",
                    sv_decl_type(ctx, decl.result(ctx), "reg"),
                    target
                ));
            }
        } else if let Some(assign) = Operation::get_op::<AssignOp>(op_ptr, ctx) {
            let target = assign.target(ctx).as_ref().to_string();
            if declared_names.insert(target.clone()) {
                let op = assign.get_operation().deref(ctx);
                declarations.push(format!(
                    "{} {};",
                    sv_decl_type(ctx, op.get_operand(0), "wire"),
                    target
                ));
            }
        } else if let Some(always) = Operation::get_op::<AlwaysFfOp>(op_ptr, ctx) {
            let target = always
                .get_attr_ff_target(ctx)
                .expect("verified always_ff target")
                .as_ref()
                .to_string();
            if declared_names.insert(target.clone()) {
                let op = always.get_operation().deref(ctx);
                declarations.push(format!(
                    "{} {};",
                    sv_decl_type(ctx, op.get_operand(1), "logic"),
                    target
                ));
            }
        } else if let Some(always) = Operation::get_op::<AlwaysFfNoResetOp>(op_ptr, ctx) {
            let target = always
                .get_attr_ff_nr_target(ctx)
                .expect("verified always_ff_no_reset target")
                .as_ref()
                .to_string();
            if declared_names.insert(target.clone()) {
                let op = always.get_operation().deref(ctx);
                declarations.push(format!(
                    "{} {};",
                    sv_decl_type(ctx, op.get_operand(1), "logic"),
                    target
                ));
            }
        } else if let Some(read) = Operation::get_op::<MemReadOp>(op_ptr, ctx) {
            let target = read
                .get_attr_memory_read_target(ctx)
                .expect("verified read target")
                .as_ref()
                .to_string();
            if declared_names.insert(target.clone()) {
                declarations.push(format!(
                    "{} {};",
                    sv_decl_type(ctx, read.result(ctx), "logic"),
                    target
                ));
            }
        } else if let Some(comb) = Operation::get_op::<AlwaysCombOp>(op_ptr, ctx) {
            let target = comb
                .get_attr_comb_target(ctx)
                .expect("verified comb target")
                .as_ref()
                .to_string();
            if declared_names.insert(target.clone()) {
                declarations.push(format!(
                    "{} {};",
                    sv_decl_type(ctx, comb.get_operation().deref(ctx).get_operand(0), "logic"),
                    target
                ));
            }
        }

        if Operation::is_op::<LogicDeclOp>(op_ptr, ctx)
            || Operation::is_op::<WireDeclOp>(op_ptr, ctx)
            || Operation::is_op::<RegDeclOp>(op_ptr, ctx)
        {
            continue;
        }

        if let Some(statement) = render_op(ctx, op_ptr) {
            statements.push(statement);
        } else if !Operation::is_op::<crate::hw::ops::OutputOp>(op_ptr, ctx)
            && !Operation::is_op::<CompRegOp>(op_ptr, ctx)
            && !Operation::is_op::<FirRegOp>(op_ptr, ctx)
            && !Operation::is_op::<HLMemOp>(op_ptr, ctx)
            && !Operation::is_op::<HLMemReadOp>(op_ptr, ctx)
            && !Operation::is_op::<HLMemWriteOp>(op_ptr, ctx)
            && !Operation::is_op::<BinaryExprOp>(op_ptr, ctx)
            && !Operation::is_op::<UnaryExprOp>(op_ptr, ctx)
            && !Operation::is_op::<ConstantExprOp>(op_ptr, ctx)
            && !Operation::is_op::<MuxExprOp>(op_ptr, ctx)
            && !Operation::is_op::<ConcatExprOp>(op_ptr, ctx)
            && !Operation::is_op::<SliceExprOp>(op_ptr, ctx)
            && !Operation::is_op::<IndexExprOp>(op_ptr, ctx)
            && !Operation::is_op::<CaseOp>(op_ptr, ctx)
        {
            return verify_err!(
                op_ptr.deref(ctx).loc(),
                "SV printer does not support operation in module"
            );
        }
    }
    for declaration in declarations {
        writeln!(output, "{}", declaration).unwrap();
    }
    for statement in statements {
        writeln!(output, "{}", statement).unwrap();
    }
    output.push_str("endmodule\n");
    Ok(output)
}

// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Deterministic SystemVerilog emission for the implemented `sv` operations.

use std::fmt::Write;

use pliron::{
    builtin::{op_interfaces::SymbolOpInterface, types::IntegerType},
    common_traits::Named,
    context::Context,
    linked_list::ContainsLinkedList,
    op::{Op, verify_op},
    operation::Operation,
    printable::Printable,
    result::Result,
    r#type::Typed,
    value::Value,
};

use crate::{
    hw::ops::ModuleOp,
    seq::types::{ClockType, ResetType},
    sv::ops::{AlwaysFfNoResetOp, AlwaysFfOp, AssignOp},
};

fn value_name(ctx: &Context, value: Value) -> String {
    value
        .given_name(ctx)
        .map(|name| name.to_string())
        .unwrap_or_else(|| value.unique_name(ctx).to_string())
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

fn render_op(ctx: &Context, op_ptr: pliron::context::Ptr<Operation>) -> Option<String> {
    if let Some(assign) = Operation::get_op::<AssignOp>(op_ptr, ctx) {
        let op = assign.get_operation().deref(ctx);
        let value = op.get_operand(0);
        return Some(format!(
            "assign {} = {};",
            assign.target(ctx).as_ref(),
            value_name(ctx, value)
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
            value_name(ctx, op.get_operand(1))
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
            value_name(ctx, op.get_operand(3)),
            always
                .get_attr_ff_target(ctx)
                .expect("verified always_ff target")
                .as_ref(),
            value_name(ctx, op.get_operand(1))
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
    let mut statements = Vec::new();
    for op_ptr in module.get_body(ctx).deref(ctx).iter(ctx) {
        if let Some(assign) = Operation::get_op::<AssignOp>(op_ptr, ctx) {
            let op = assign.get_operation().deref(ctx);
            declarations.push(format!(
                "{} {};",
                sv_decl_type(ctx, op.get_operand(0), "wire"),
                assign.target(ctx).as_ref()
            ));
        } else if let Some(always) = Operation::get_op::<AlwaysFfOp>(op_ptr, ctx) {
            let op = always.get_operation().deref(ctx);
            declarations.push(format!(
                "{} {};",
                sv_decl_type(ctx, op.get_operand(1), "logic"),
                always
                    .get_attr_ff_target(ctx)
                    .expect("verified always_ff target")
                    .as_ref()
            ));
        } else if let Some(always) = Operation::get_op::<AlwaysFfNoResetOp>(op_ptr, ctx) {
            let op = always.get_operation().deref(ctx);
            declarations.push(format!(
                "{} {};",
                sv_decl_type(ctx, op.get_operand(1), "logic"),
                always
                    .get_attr_ff_nr_target(ctx)
                    .expect("verified always_ff_no_reset target")
                    .as_ref()
            ));
        }
        if let Some(statement) = render_op(ctx, op_ptr) {
            statements.push(statement);
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

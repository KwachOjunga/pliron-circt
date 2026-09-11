// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Typed lowering helpers from semantic hardware dialects into `sv` intent.

use pliron::{
    builtin::attributes::StringAttr,
    common_traits::{Named, Verify},
    context::Context,
    linked_list::ContainsLinkedList,
    op::{Op, verify_op},
    operation::Operation,
    result::Result,
    value::Value,
    verify_err,
};

use crate::{
    hw::ops::{ModuleOp, OutputOp},
    seq::ops::{CompRegOp, FirRegOp},
    sv::ops::{AlwaysFfNoResetOp, AlwaysFfOp, AssignOp},
};

enum RegisterPlan {
    Plain {
        clock: Value,
        input: Value,
        target: String,
    },
    Reset {
        clock: Value,
        input: Value,
        reset: Value,
        reset_value: Value,
        target: String,
        is_async_reset: bool,
        reset_polarity: String,
    },
}

fn result_target(ctx: &Context, value: Value, ordinal: usize) -> String {
    value
        .given_name(ctx)
        .map(|name| name.to_string())
        .unwrap_or_else(|| format!("state_{}", ordinal))
}

/// Lower a verified combinational value to a named continuous assignment.
pub fn lower_assign(ctx: &mut Context, target: impl Into<StringAttr>, value: Value) -> AssignOp {
    AssignOp::new(ctx, target, value)
}

/// Lower a plain `seq.compreg` to reset-free `sv.always_ff` intent.
pub fn lower_compreg(
    ctx: &mut Context,
    source: &CompRegOp,
    target: impl Into<StringAttr>,
) -> AlwaysFfNoResetOp {
    AlwaysFfNoResetOp::new(ctx, target, source.clock(ctx), source.input(ctx))
}

/// Lower a verified reset-bearing register to reset-aware `sv.always_ff` intent.
pub fn lower_firreg(
    ctx: &mut Context,
    source: &FirRegOp,
    target: impl Into<StringAttr>,
) -> AlwaysFfOp {
    AlwaysFfOp::new(
        ctx,
        target,
        source.clock(ctx),
        source.input(ctx),
        source.reset(ctx),
        source.reset_value(ctx),
        source.is_async_reset(ctx),
        source.reset_polarity(ctx),
    )
}

/// Convert all supported sequential operations in a module to SV operations.
///
/// The source operations remain in the module so later passes can compare the
/// semantic IR with its emission intent. Generated operations are inserted
/// immediately before `hw.output`, preserving the module terminator rule.
/// Unsupported operations are left untouched and are reported by the returned
/// conversion count only through the operations that were converted.
pub fn lower_module_registers(ctx: &mut Context, module: &ModuleOp) -> Result<usize> {
    verify_op(module, ctx)?;
    let body = module.get_body(ctx);
    let output = match body
        .deref(ctx)
        .iter(ctx)
        .find(|op| Operation::is_op::<OutputOp>(*op, ctx))
    {
        Some(output) => output,
        None => {
            return verify_err!(
                module.loc(ctx),
                "SV lowering requires an hw.output terminator"
            );
        }
    };

    let mut plans = Vec::new();
    let mut ordinal = 0;
    for op_ptr in body.deref(ctx).iter(ctx) {
        if let Some(compreg) = Operation::get_op::<CompRegOp>(op_ptr, ctx) {
            compreg.verify(ctx)?;
            plans.push(RegisterPlan::Plain {
                clock: compreg.clock(ctx),
                input: compreg.input(ctx),
                target: result_target(ctx, compreg.result(ctx), ordinal),
            });
            ordinal += 1;
        } else if let Some(firreg) = Operation::get_op::<FirRegOp>(op_ptr, ctx) {
            firreg.verify(ctx)?;
            plans.push(RegisterPlan::Reset {
                clock: firreg.clock(ctx),
                input: firreg.input(ctx),
                reset: firreg.reset(ctx),
                reset_value: firreg.reset_value(ctx),
                target: result_target(ctx, firreg.result(ctx), ordinal),
                is_async_reset: firreg.is_async_reset(ctx),
                reset_polarity: firreg.reset_polarity(ctx).as_ref().to_owned(),
            });
            ordinal += 1;
        }
    }

    let mut converted = 0;
    for plan in plans {
        match plan {
            RegisterPlan::Plain {
                clock,
                input,
                target,
            } => {
                let lowered = AlwaysFfNoResetOp::new(ctx, target, clock, input);
                lowered.get_operation().insert_before(ctx, output);
            }
            RegisterPlan::Reset {
                clock,
                input,
                reset,
                reset_value,
                target,
                is_async_reset,
                reset_polarity,
            } => {
                let lowered = AlwaysFfOp::new(
                    ctx,
                    target,
                    clock,
                    input,
                    reset,
                    reset_value,
                    is_async_reset,
                    reset_polarity,
                );
                lowered.get_operation().insert_before(ctx, output);
            }
        }
        converted += 1;
    }
    Ok(converted)
}

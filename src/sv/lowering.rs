// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Typed lowering helpers from semantic hardware dialects into `sv` intent.

use pliron::{builtin::attributes::StringAttr, context::Context, value::Value};

use crate::{
    seq::ops::{CompRegOp, FirRegOp},
    sv::ops::{AlwaysFfNoResetOp, AlwaysFfOp, AssignOp},
};

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

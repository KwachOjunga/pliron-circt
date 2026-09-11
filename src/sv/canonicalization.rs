// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Semantics-preserving canonicalization patterns for `sv` operations.

use pliron::{
    common_traits::{Named, Verify},
    context::Context,
    irbuild::rewriter::Rewriter,
    op::Op,
    result::Result,
    value::Value,
};

use super::ops::AssignOp;

/// Replace a named assignment whose target is already the source value name.
///
/// This is the concrete canonicalization pattern for:
///
/// ```text
/// %x = sv.assign %x {assign_target = "x"}
/// ```
///
/// The assignment contributes no semantic conversion in that case. The
/// caller's rewriter replaces its result uses with the original source value
/// and erases the redundant operation.
pub fn eliminate_redundant_assign(
    ctx: &mut Context,
    rewriter: &mut impl Rewriter,
    assign: &AssignOp,
) -> Result<bool> {
    assign.verify(ctx)?;
    let source = assign.get_operation().deref(ctx).get_operand(0);
    let source_name = match source.given_name(ctx) {
        Some(name) => name,
        None => return Ok(false),
    };
    if assign.target(ctx).as_ref() != source_name.as_ref() {
        return Ok(false);
    }
    rewriter.replace_operation_with_values(ctx, assign.get_operation(), vec![source]);
    Ok(true)
}

/// Return the source value of an assignment for analysis-only canonicalizers.
pub fn assignment_source(ctx: &Context, assign: &AssignOp) -> Value {
    assign.get_operation().deref(ctx).get_operand(0)
}

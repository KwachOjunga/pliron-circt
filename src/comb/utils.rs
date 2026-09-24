use pliron::{
    context::Context,
    location::Located,
    operation::Operation,
    result::Result,
    r#type::{TypeHandle, Typed},
    value::Value,
    verify_err,
};

use crate::comb::types::{get_integer_width, verify_i1, verify_integer_type};

// -----------------------------------------------------------------------------
// Common Verification Helpers
// -----------------------------------------------------------------------------

pub fn verify_binary_arithmetic(op: &Operation, ctx: &Context, op_name: &str) -> Result<()> {
    let lhs_w = verify_integer_type(
        op,
        ctx,
        op.get_operand(0).get_type(ctx),
        &format!("{} lhs", op_name),
    )?;
    let rhs_w = verify_integer_type(
        op,
        ctx,
        op.get_operand(1).get_type(ctx),
        &format!("{} rhs", op_name),
    )?;
    if lhs_w != rhs_w {
        return verify_err!(
            op.loc(),
            "{} operand width mismatch: lhs has width {}, rhs has width {}",
            op_name,
            lhs_w,
            rhs_w
        );
    }
    let res_w = verify_integer_type(
        op,
        ctx,
        op.get_result(0).get_type(ctx),
        &format!("{} result", op_name),
    )?;
    if res_w != lhs_w {
        return verify_err!(
            op.loc(),
            "{} result width mismatch: expected width {}, found {}",
            op_name,
            lhs_w,
            res_w
        );
    }
    Ok(())
}

pub fn verify_unary(op: &Operation, ctx: &Context, op_name: &str) -> Result<()> {
    let in_w = verify_integer_type(
        op,
        ctx,
        op.get_operand(0).get_type(ctx),
        &format!("{} input", op_name),
    )?;
    let res_w = verify_integer_type(
        op,
        ctx,
        op.get_result(0).get_type(ctx),
        &format!("{} result", op_name),
    )?;
    if res_w != in_w {
        return verify_err!(
            op.loc(),
            "{} result width mismatch: expected width {}, found {}",
            op_name,
            in_w,
            res_w
        );
    }
    Ok(())
}

pub fn verify_reduction(op: &Operation, ctx: &Context, op_name: &str) -> Result<()> {
    let _in_w = verify_integer_type(
        op,
        ctx,
        op.get_operand(0).get_type(ctx),
        &format!("{} input", op_name),
    )?;
    verify_i1(
        op,
        ctx,
        op.get_result(0).get_type(ctx),
        &format!("{} result", op_name),
    )
}

pub fn verify_variadic_bitwise(op: &Operation, ctx: &Context, op_name: &str) -> Result<()> {
    if op.get_num_operands() == 0 {
        return verify_err!(
            op.loc(),
            "{} requires at least one operand, found 0",
            op_name
        );
    }
    let first_w = verify_integer_type(
        op,
        ctx,
        op.get_operand(0).get_type(ctx),
        &format!("{} operand 0", op_name),
    )?;
    for i in 1..op.get_num_operands() {
        let opd_w = verify_integer_type(
            op,
            ctx,
            op.get_operand(i).get_type(ctx),
            &format!("{} operand {}", op_name, i),
        )?;
        if opd_w != first_w {
            return verify_err!(
                op.loc(),
                "{} operand width mismatch: operand 0 has width {}, operand {} has width {}",
                op_name,
                first_w,
                i,
                opd_w
            );
        }
    }
    let res_w = verify_integer_type(
        op,
        ctx,
        op.get_result(0).get_type(ctx),
        &format!("{} result", op_name),
    )?;
    if res_w != first_w {
        return verify_err!(
            op.loc(),
            "{} result width mismatch: expected width {}, found {}",
            op_name,
            first_w,
            res_w
        );
    }
    Ok(())
}

pub fn assert_binary_arithmetic(ctx: &Context, args: &[Value], res_ty: TypeHandle, op_name: &str) {
    let args_width = args
        .iter()
        .map(|arg| {
            get_integer_width(ctx, arg.get_type(ctx))
                .unwrap_or_else(|e| panic!("{op_name} {:#?} type error: {e}", arg))
        })
        .collect::<Vec<_>>();
    let first_w = args_width[0];
    for (i, arg_w) in args_width.iter().enumerate().skip(1) {
        assert_eq!(
            arg_w, &first_w,
            "{op_name} operand width mismatch: arg {i} has width {arg_w}, first arg has width {first_w}"
        );
    }

    let w_res = get_integer_width(ctx, res_ty)
        .unwrap_or_else(|e| panic!("{op_name} result type error: {e}"));
    assert_eq!(
        w_res, first_w,
        "{op_name} result width mismatch: expected width {first_w}, found {w_res}"
    );
}

#[allow(dead_code)]
fn assert_unary(ctx: &Context, val: Value, res_ty: TypeHandle, op_name: &str) {
    let in_w = get_integer_width(ctx, val.get_type(ctx))
        .unwrap_or_else(|e| panic!("{op_name} input type error: {e}"));
    let res_w = get_integer_width(ctx, res_ty)
        .unwrap_or_else(|e| panic!("{op_name} result type error: {e}"));
    assert_eq!(
        res_w, in_w,
        "{op_name} result width mismatch: expected width {in_w}, found {res_w}"
    );
}

#[allow(dead_code)]
fn assert_reduction(ctx: &Context, val: Value, i1_ty: TypeHandle, op_name: &str) {
    let _in_w = get_integer_width(ctx, val.get_type(ctx))
        .unwrap_or_else(|e| panic!("{op_name} input type error: {e}"));
    let res_w = get_integer_width(ctx, i1_ty)
        .unwrap_or_else(|e| panic!("{op_name} result type error: {e}"));
    assert_eq!(res_w, 1, "{op_name} result must be i1, found width {res_w}");
}

#[allow(dead_code)]
fn assert_variadic_bitwise(ctx: &Context, inputs: &[Value], res_ty: TypeHandle, op_name: &str) {
    assert!(!inputs.is_empty(), "{op_name} requires at least one input");
    let w0 = get_integer_width(ctx, inputs[0].get_type(ctx))
        .unwrap_or_else(|e| panic!("{op_name} input 0 type error: {e}"));
    for (i, inp) in inputs.iter().enumerate().skip(1) {
        let wi = get_integer_width(ctx, inp.get_type(ctx))
            .unwrap_or_else(|e| panic!("{op_name} input {i} type error: {e}"));
        assert_eq!(
            wi, w0,
            "{op_name} input {i} width ({wi}) != input 0 width ({w0})"
        );
    }
    let w_res = get_integer_width(ctx, res_ty)
        .unwrap_or_else(|e| panic!("{op_name} result type error: {e}"));
    assert_eq!(
        w_res, w0,
        "{op_name} result width ({w_res}) != input width ({w0})"
    );
}

// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Type validation and metadata utilities for the `comb` dialect.

use pliron::{
    builtin::types::{IntegerType, Signedness},
    context::Context,
    location::Located,
    operation::Operation,
    printable::Printable,
    result::Result,
    r#type::TypeHandle,
    verify_err, verify_err_noloc,
};

/// Retrieve the bitwidth of a signless `IntegerType`.
///
/// Returns an error if the type is not a signless `IntegerType` or if its width is zero.
pub fn get_integer_width(ctx: &Context, ty: TypeHandle) -> Result<u32> {
    let ty_ref = ty.deref(ctx);
    if let Some(int_ty) = ty_ref.downcast_ref::<IntegerType>() {
        if int_ty.signedness() != Signedness::Signless {
            return verify_err_noloc!("comb integer type must be signless, found {}", ty.disp(ctx));
        }
        let w = int_ty.width();
        if w == 0 {
            return verify_err_noloc!("zero-width integer is illegal in comb dialect");
        }
        Ok(w)
    } else {
        verify_err_noloc!(
            "expected integer type in comb dialect, found {}",
            ty.disp(ctx)
        )
    }
}

/// Verify that `ty` is a valid signless `IntegerType` with non-zero width on an operation.
pub fn verify_integer_type(
    op: &Operation,
    ctx: &Context,
    ty: TypeHandle,
    desc: &str,
) -> Result<u32> {
    let ty_ref = ty.deref(ctx);
    if let Some(int_ty) = ty_ref.downcast_ref::<IntegerType>() {
        if int_ty.signedness() != Signedness::Signless {
            return verify_err!(
                op.loc(),
                "{} must have signless integer type, found {}",
                desc,
                ty.disp(ctx)
            );
        }
        let w = int_ty.width();
        if w == 0 {
            return verify_err!(
                op.loc(),
                "{} has zero-width integer, which is illegal in comb dialect",
                desc
            );
        }
        Ok(w)
    } else {
        verify_err!(
            op.loc(),
            "{} expected integer type, found {}",
            desc,
            ty.disp(ctx)
        )
    }
}

/// Verify that `ty` is a 1-bit signless integer (`i1`).
pub fn verify_i1(op: &Operation, ctx: &Context, ty: TypeHandle, desc: &str) -> Result<()> {
    let w = verify_integer_type(op, ctx, ty, desc)?;
    if w != 1 {
        return verify_err!(
            op.loc(),
            "{} expected 1-bit integer (i1), found width {}",
            desc,
            w
        );
    }
    Ok(())
}

/// Verify that two types are integer types with identical non-zero width.
pub fn verify_same_integer_width(
    op: &Operation,
    ctx: &Context,
    a: TypeHandle,
    b: TypeHandle,
    a_desc: &str,
    b_desc: &str,
) -> Result<u32> {
    let wa = verify_integer_type(op, ctx, a, a_desc)?;
    let wb = verify_integer_type(op, ctx, b, b_desc)?;
    if wa != wb {
        return verify_err!(
            op.loc(),
            "width mismatch: {} has width {}, but {} has width {}",
            a_desc,
            wa,
            b_desc,
            wb
        );
    }
    Ok(wa)
}

/// Trait providing standard hardware property queries across `comb` operations.
pub trait CombOpExt {
    /// Return the primary result bitwidth.
    fn width(&self, ctx: &Context) -> u32;

    /// Return whether this operation interprets its inputs as signed.
    fn is_signed(&self) -> bool {
        false
    }

    /// Return whether this operation is pure combinational logic (always true for comb).
    fn is_pure(&self) -> bool {
        true
    }

    /// Return the static latency of this operation in clock cycles (always 0 for comb).
    fn latency(&self) -> u32 {
        0
    }
}

// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Canonicalization and simplification rewrite rules for the `comb` dialect.
//!
//! Implements algebraic simplifications and canonical forms:
//! - Commutative operand ordering (constants normalized to the right)
//! - Subtraction by constant rewritten as addition of two's-complement negation
//! - Mux identity simplifications: `mux(c, v, v) -> v`, `mux(c, 1, 0) -> c`
//! - Identity element simplifications: `add(x, 0) -> x`, `mul(x, 1) -> x`, etc.
#![allow(unused)]
use core::num::NonZero;

use pliron::{
    basic_block::BasicBlock,
    builtin::{
        attributes::IntegerAttr,
        types::{IntegerType, Signedness},
    },
    context::{Context, Ptr},
    linked_list::ContainsLinkedList,
    op::Op,
    operation::Operation,
    result::Result,
    r#type::Typed,
    utils::apint::APInt,
    value::Value,
};

use crate::{
    comb::{
        eval::{eval_neg, get_constant_value},
        ops::{AddOp, ExtractOp, MuxOp, ReplicateOp},
    },
    hw::ops::ConstantOp,
};

/// Result of attempting to canonicalize an operation.
#[derive(Debug, PartialEq, Eq)]
pub enum CanonicalizationResult {
    /// No change was made.
    Unchanged,
    /// The operation was simplified or normalized in-place.
    Modified,
    /// The operation's result was replaced by an existing SSA value.
    Replaced(Value),
}

/// Normalize commutative operands: constants are placed on the right (index 1).
pub fn canonicalize_commutative(ctx: &mut Context, op_ptr: Ptr<Operation>) -> bool {
    let op = op_ptr.deref(ctx);
    let name = Operation::get_opid(op_ptr, ctx).name;
    let is_commutative = matches!(
        name.as_ref(),
        "comb.add" | "comb.mul" | "comb.and" | "comb.or" | "comb.xor"
    );
    if !is_commutative || op.get_num_operands() != 2 {
        return false;
    }

    let opd0 = op.get_operand(0);
    let opd1 = op.get_operand(1);
    let c0 = get_constant_value(ctx, opd0);
    let c1 = get_constant_value(ctx, opd1);

    // If lhs is constant and rhs is not, swap them to place constant on the right
    if c0.is_some() && c1.is_none() {
        Operation::insert_operand(op_ptr, ctx, 0, opd1);
        Operation::insert_operand(op_ptr, ctx, 1, opd0);
        true
    } else {
        false
    }
}

/// Simplify subtraction of a constant: `sub(x, C) -> add(x, -C)`.
// pub fn canonicalize_sub_to_add(ctx: &mut Context, op_ptr: Ptr<Operation>) -> Option<AddOp> {
//     let op = op_ptr.clone().deref(ctx);
//     let name = Operation::get_opid(op_ptr, ctx).name;
//     let name_str: &str = name.as_ref();
//     if name_str != "comb.sub" {
//         return None;
//     }

//     let lhs = op.get_operand(0);
//     let rhs = op.get_operand(1);
//     let rhs_const = get_constant_value(ctx, rhs)?;

//     // Compute two's-complement negation: -C
//     let neg_c = eval_neg(&rhs_const);
//     let ty = rhs.get_type(ctx);
//     let w = neg_c.bw() as u32;
//     let signless_ty = IntegerType::get(ctx, w, Signedness::Signless);
//     let neg_attr = IntegerAttr::new(signless_ty, neg_c);

//     let new_const = ConstantOp::new(ctx, neg_attr);
//     // let parent_block = op_ptr.deref(ctx).get_parent_block().expect("parent block");
//     new_const.get_operation().insert_before(ctx, op_ptr);
//     let add_op_res = new_const.result(ctx);
//     let add_op = AddOp::new(ctx, lhs, add_op_res, ty);
//     add_op.get_operation().insert_before(ctx, op_ptr);

//     let old_res = op_ptr.deref(ctx).get_result(0);
//     old_res.replace_some_uses_with(ctx, |_, _| true, &add_op.result(ctx));
//     Operation::erase(op_ptr, ctx);

//     Some(add_op)
// }

pub fn canonicalize_sub_to_add(ctx: &mut Context, op_ptr: Ptr<Operation>) -> Option<AddOp> {
    let name = Operation::get_opid(op_ptr, ctx).name;
    let name_str: &str = name.as_ref();

    if name_str != "comb.sub" {
        return None;
    }

    let (lhs, rhs, ty) = {
        let op = op_ptr.clone().deref(ctx);

        (
            op.get_operand(0),
            op.get_operand(1),
            op.get_result(0).get_type(ctx),
        )
    };

    let rhs_const = get_constant_value(ctx, rhs)?;
    let neg_c = eval_neg(&rhs_const);
    let w = neg_c.bw() as u32;
    let signless_ty = IntegerType::get(ctx, w, Signedness::Signless);
    let neg_attr = IntegerAttr::new(signless_ty, neg_c);
    let new_const = ConstantOp::new(ctx, neg_attr);
    new_const.get_operation().insert_before(ctx, op_ptr);
    let add_op_res = new_const.result(ctx);
    let add_op = AddOp::new(ctx, lhs, add_op_res, ty);
    add_op.get_operation().insert_before(ctx, op_ptr);
    let old_res = op_ptr.deref(ctx).get_result(0);
    old_res.replace_some_uses_with(ctx, |_, _| true, &add_op.result(ctx));
    Operation::erase(op_ptr, ctx);
    Some(add_op)
}

/// Simplify `comb.mux`:
/// - `mux(c, v, v) -> v`
/// - `mux(c, 1, 0) -> c` (when c is i1, true is 1, false is 0)
pub fn canonicalize_mux(ctx: &mut Context, mux_op: MuxOp) -> Option<Value> {
    let op = mux_op.get_operation().deref(ctx);
    let cond = op.get_operand(0);
    let true_val = op.get_operand(1);
    let false_val = op.get_operand(2);

    // Rule 1: mux(c, v, v) -> v
    if true_val == false_val {
        return Some(true_val);
    }

    // Rule 2: mux(c, 1, 0) -> c (for i1)
    let c_true = get_constant_value(ctx, true_val);
    let c_false = get_constant_value(ctx, false_val);
    if let (Some(t_val), Some(f_val)) = (c_true, c_false) {
        let one_bit = NonZero::new(1).unwrap();
        if t_val.bw() == 1 && f_val.bw() == 1 {
            let one = APInt::uone(one_bit);
            let zero = APInt::zero(one_bit);
            if t_val == one && f_val == zero {
                return Some(cond);
            }
        }
    }

    None
}

/// Simplify identity elements:
/// - `add(x, 0) -> x`
/// - `mul(x, 1) -> x`
/// - `mul(x, 0) -> 0`
/// - `and(x, all_ones) -> x`
/// - `and(x, 0) -> 0`
/// - `or(x, 0) -> x`
/// - `xor(x, 0) -> x`
/// - `xor(x, x) -> 0`
/// - `extract(x, 0, W) where W == x.width -> x`
/// - `replicate(x, 1) -> x`
pub fn canonicalize_identities(ctx: &mut Context, op_ptr: Ptr<Operation>) -> Option<Value> {
    let name = Operation::get_opid(op_ptr, ctx).name;
    let name_str: &str = name.as_ref();

    match name_str {
        "comb.add" => {
            let (lhs, rhs) = {
                let op = op_ptr.clone().deref(ctx);
                (op.get_operand(0), op.get_operand(1))
            };
            if let Some(c) = get_constant_value(ctx, rhs) {
                if c.is_zero() {
                    return Some(lhs);
                }
            }
            if let Some(c) = get_constant_value(ctx, lhs) {
                if c.is_zero() {
                    return Some(rhs);
                }
            }
        }
        "comb.mul" => {
            let (lhs, rhs) = {
                let op = op_ptr.clone().deref(ctx);
                (op.get_operand(0), op.get_operand(1))
            };
            let w = NonZero::new(
                lhs.get_type(ctx)
                    .deref(ctx)
                    .downcast_ref::<IntegerType>()?
                    .width() as usize,
            )?;
            let one = APInt::uone(w);
            if let Some(c) = get_constant_value(ctx, rhs) {
                if c == one {
                    return Some(lhs);
                }
                if c.is_zero() {
                    return Some(rhs);
                }
            }
            if let Some(c) = get_constant_value(ctx, lhs) {
                if c == one {
                    return Some(rhs);
                }
                if c.is_zero() {
                    return Some(lhs);
                }
            }
        }
        "comb.and" => {
            let op = op_ptr.clone().deref(ctx);
            let (lhs, rhs) = { (op.get_operand(0), op.get_operand(1)) };
            if op.get_num_operands() == 2 {
                let w = NonZero::new(
                    lhs.get_type(ctx)
                        .deref(ctx)
                        .downcast_ref::<IntegerType>()?
                        .width() as usize,
                )?;
                let all_ones = APInt::umax(w);
                if let Some(c) = get_constant_value(ctx, rhs) {
                    if c == all_ones {
                        return Some(lhs);
                    }
                    if c.is_zero() {
                        return Some(rhs);
                    }
                }
                if let Some(c) = get_constant_value(ctx, lhs) {
                    if c == all_ones {
                        return Some(rhs);
                    }
                    if c.is_zero() {
                        return Some(lhs);
                    }
                }
            }
        }
        "comb.or" => {
            let op = op_ptr.clone().deref(ctx);
            let (lhs, rhs) = { (op.get_operand(0), op.get_operand(1)) };
            if op.get_num_operands() == 2 {
                if let Some(c) = get_constant_value(ctx, rhs) {
                    if c.is_zero() {
                        return Some(lhs);
                    }
                }
                if let Some(c) = get_constant_value(ctx, lhs) {
                    if c.is_zero() {
                        return Some(rhs);
                    }
                }
            }
        }
        "comb.xor" => {
            let (lhs, rhs, num_operands) = {
                let op = op_ptr.clone().deref(ctx);
                (op.get_operand(0), op.get_operand(1), op.get_num_operands())
            };
            if num_operands == 2 {
                // xor(x, x) -> 0
                if lhs == rhs {
                    let w = lhs
                        .get_type(ctx)
                        .deref(ctx)
                        .downcast_ref::<IntegerType>()?
                        .width();
                    let zero_attr = IntegerAttr::new(
                        IntegerType::get(ctx, w, Signedness::Signless),
                        APInt::zero(NonZero::new(w as usize)?),
                    );
                    let zero_const = ConstantOp::new(ctx, zero_attr);
                    zero_const.get_operation().insert_before(ctx, op_ptr);
                    return Some(zero_const.result(ctx));
                }
                if let Some(c) = get_constant_value(ctx, rhs) {
                    if c.is_zero() {
                        return Some(lhs);
                    }
                }
                if let Some(c) = get_constant_value(ctx, lhs) {
                    if c.is_zero() {
                        return Some(rhs);
                    }
                }
            }
        }
        "comb.extract" => {
            let ext = Operation::get_op::<ExtractOp>(op_ptr, ctx)?;
            let low_bit = ext.low_bit(ctx).value().to_u64();
            let in_w = ext
                .val(ctx)
                .get_type(ctx)
                .deref(ctx)
                .downcast_ref::<IntegerType>()?
                .width();
            let res_w = ext
                .result(ctx)
                .get_type(ctx)
                .deref(ctx)
                .downcast_ref::<IntegerType>()?
                .width();
            if low_bit == 0 && in_w == res_w {
                return Some(ext.val(ctx));
            }
        }
        "comb.replicate" => {
            let rep = Operation::get_op::<ReplicateOp>(op_ptr, ctx)?;
            let count = rep.count(ctx).value().to_u64();
            if count == 1 {
                return Some(rep.val(ctx));
            }
        }
        _ => {}
    }

    None
}

/// Driver that applies canonicalization passes across an entire basic block.
///
/// Returns the number of simplifications performed.
pub fn canonicalize_block(ctx: &mut Context, block: Ptr<BasicBlock>) -> Result<usize> {
    let mut count = 0;
    let mut changed = true;

    while changed {
        changed = false;
        let ops: Vec<Ptr<Operation>> = block.deref(ctx).iter(ctx).collect();

        for op_ptr in ops {
            // Check if op is still in the block
            if op_ptr.deref(ctx).get_parent_block() != Some(block) {
                continue;
            }

            // 1. Commutative normalization
            if canonicalize_commutative(ctx, op_ptr) {
                changed = true;
                count += 1;
            }

            // 2. Identity simplifications
            if let Some(replacement) = canonicalize_identities(ctx, op_ptr) {
                let res = op_ptr.deref(ctx).get_result(0);
                res.replace_some_uses_with(ctx, |_, _| true, &replacement);
                Operation::erase(op_ptr, ctx);
                changed = true;
                count += 1;
                continue;
            }

            // 3. Mux simplifications
            if let Some(mux_op) = Operation::get_op::<MuxOp>(op_ptr, ctx) {
                if let Some(replacement) = canonicalize_mux(ctx, mux_op) {
                    let res = op_ptr.deref(ctx).get_result(0);
                    res.replace_some_uses_with(ctx, |_, _| true, &replacement);
                    Operation::erase(op_ptr, ctx);
                    changed = true;
                    count += 1;
                    continue;
                }
            }

            // 4. Sub to add rewriting
            if canonicalize_sub_to_add(ctx, op_ptr).is_some() {
                changed = true;
                count += 1;
                continue;
            }
        }
    }

    Ok(count)
}

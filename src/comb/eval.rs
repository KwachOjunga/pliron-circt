// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Executable bit-level semantics, evaluation, and constant folding for `comb` operations.
//!
//! Provides mathematically precise evaluation matching the hardware specification:
//! - Modular overflow for `add`, `sub`, `mul`
//! - Signed vs unsigned division and remainder (`divu`, `divs`, `modu`, `mods`)
//! - Explicit undefined handling for division and remainder by zero (never guessed)
//! - Shifts with amounts >= width (0 for logical, sign-extension for arithmetic)
//! - Bit-exact ordering for concatenation (first operand high, last operand low)
//! - Bit slice extraction and replication
#![allow(unused)]
use core::num::NonZero;

use awint::Awi;
use pliron::{
    builtin::types::IntegerType,
    context::{Context, Ptr},
    operation::Operation,
    r#type::Typed,
    utils::apint::APInt,
    value::Value,
};

use crate::{
    comb::ops::{ExtractOp, ICmpOp, ICmpPredicate, ReplicateOp},
    hw::ops::ConstantOp,
};

/// Errors encountered during evaluation of combinational operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvalError {
    /// Division by zero is undefined in hardware and cannot be folded.
    DivisionByZero,
    /// Remainder by zero is undefined in hardware and cannot be folded.
    RemainderByZero,
    /// Bitwidth mismatch between operands or results.
    BitWidthMismatch(String),
    /// An operation requires non-zero bitwidth.
    ZeroWidth,
    /// Extraction or slice bounds exceeded the input width.
    OutOfBounds(String),
    /// An empty operand list was passed to a variadic operation.
    EmptyOperands,
    /// Unknown or unsupported operation.
    UnsupportedOperation(String),
}

impl core::fmt::Display for EvalError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::DivisionByZero => write!(f, "division by zero is undefined"),
            Self::RemainderByZero => write!(f, "remainder by zero is undefined"),
            Self::BitWidthMismatch(msg) => write!(f, "bitwidth mismatch: {}", msg),
            Self::ZeroWidth => write!(f, "bitwidth must be greater than zero"),
            Self::OutOfBounds(msg) => write!(f, "out of bounds: {}", msg),
            Self::EmptyOperands => write!(f, "empty operands list"),
            Self::UnsupportedOperation(name) => write!(f, "unsupported operation: {}", name),
        }
    }
}

// -----------------------------------------------------------------------------
// Bit-level Evaluation Functions
// -----------------------------------------------------------------------------

/// Evaluate `comb.add`: $(lhs + rhs) \pmod{2^W}$.
pub fn eval_add(lhs: &APInt, rhs: &APInt) -> Result<APInt, EvalError> {
    if lhs.bw() != rhs.bw() {
        return Err(EvalError::BitWidthMismatch(
            "add operands must have equal width".into(),
        ));
    }
    Ok(lhs.add(rhs))
}

/// Evaluate `comb.sub`: $(lhs - rhs) \pmod{2^W}$.
pub fn eval_sub(lhs: &APInt, rhs: &APInt) -> Result<APInt, EvalError> {
    if lhs.bw() != rhs.bw() {
        return Err(EvalError::BitWidthMismatch(
            "sub operands must have equal width".into(),
        ));
    }
    Ok(lhs.sub(rhs))
}

/// Evaluate `comb.mul`: $(lhs \times rhs) \pmod{2^W}$.
pub fn eval_mul(lhs: &APInt, rhs: &APInt) -> Result<APInt, EvalError> {
    if lhs.bw() != rhs.bw() {
        return Err(EvalError::BitWidthMismatch(
            "mul operands must have equal width".into(),
        ));
    }
    Ok(lhs.mul(rhs))
}

/// Evaluate `comb.divu`: Unsigned division.
///
/// Returns `Err(EvalError::DivisionByZero)` if divisor is zero, preserving undefined behavior.
pub fn eval_divu(lhs: &APInt, rhs: &APInt) -> Result<APInt, EvalError> {
    if lhs.bw() != rhs.bw() {
        return Err(EvalError::BitWidthMismatch(
            "divu operands must have equal width".into(),
        ));
    }
    if rhs.is_zero() {
        return Err(EvalError::DivisionByZero);
    }
    Ok(lhs.udiv(rhs))
}

/// Evaluate `comb.divs`: Signed two's-complement division.
///
/// Returns `Err(EvalError::DivisionByZero)` if divisor is zero, preserving undefined behavior.
pub fn eval_divs(lhs: &APInt, rhs: &APInt) -> Result<APInt, EvalError> {
    if lhs.bw() != rhs.bw() {
        return Err(EvalError::BitWidthMismatch(
            "divs operands must have equal width".into(),
        ));
    }
    if rhs.is_zero() {
        return Err(EvalError::DivisionByZero);
    }
    Ok(lhs.sdiv(rhs))
}

/// Evaluate `comb.modu`: Unsigned remainder.
///
/// Returns `Err(EvalError::RemainderByZero)` if divisor is zero, preserving undefined behavior.
pub fn eval_modu(lhs: &APInt, rhs: &APInt) -> Result<APInt, EvalError> {
    if lhs.bw() != rhs.bw() {
        return Err(EvalError::BitWidthMismatch(
            "modu operands must have equal width".into(),
        ));
    }
    if rhs.is_zero() {
        return Err(EvalError::RemainderByZero);
    }
    Ok(lhs.urem(rhs))
}

/// Evaluate `comb.mods`: Signed two's-complement remainder.
///
/// Returns `Err(EvalError::RemainderByZero)` if divisor is zero, preserving undefined behavior.
pub fn eval_mods(lhs: &APInt, rhs: &APInt) -> Result<APInt, EvalError> {
    if lhs.bw() != rhs.bw() {
        return Err(EvalError::BitWidthMismatch(
            "mods operands must have equal width".into(),
        ));
    }
    if rhs.is_zero() {
        return Err(EvalError::RemainderByZero);
    }
    Ok(lhs.srem(rhs))
}

/// Evaluate `comb.shl`: Logical shift left.
///
/// Shift amounts $\ge W$ produce zero.
pub fn eval_shl(val: &APInt, shift: &APInt) -> Result<APInt, EvalError> {
    if val.bw() != shift.bw() {
        return Err(EvalError::BitWidthMismatch(
            "shl operands must have equal width".into(),
        ));
    }
    Ok(val.shl(shift))
}

/// Evaluate `comb.shru`: Logical shift right.
///
/// Shift amounts $\ge W$ produce zero.
pub fn eval_shru(val: &APInt, shift: &APInt) -> Result<APInt, EvalError> {
    if val.bw() != shift.bw() {
        return Err(EvalError::BitWidthMismatch(
            "shru operands must have equal width".into(),
        ));
    }
    Ok(val.lshr(shift))
}

/// Evaluate `comb.shrs`: Arithmetic shift right.
///
/// Shift amounts $\ge W$ fill with the sign bit.
pub fn eval_shrs(val: &APInt, shift: &APInt) -> Result<APInt, EvalError> {
    if val.bw() != shift.bw() {
        return Err(EvalError::BitWidthMismatch(
            "shrs operands must have equal width".into(),
        ));
    }
    Ok(val.ashr(shift))
}

/// Evaluate `comb.and`: Bitwise AND across non-empty operands.
pub fn eval_and(inputs: &[&APInt]) -> Result<APInt, EvalError> {
    if inputs.is_empty() {
        return Err(EvalError::EmptyOperands);
    }
    let mut acc = (*inputs[0]).clone();
    for opd in &inputs[1..] {
        if acc.bw() != opd.bw() {
            return Err(EvalError::BitWidthMismatch(
                "and operands must have equal width".into(),
            ));
        }
        acc = acc.and(opd);
    }
    Ok(acc)
}

/// Evaluate `comb.or`: Bitwise OR across non-empty operands.
pub fn eval_or(inputs: &[&APInt]) -> Result<APInt, EvalError> {
    if inputs.is_empty() {
        return Err(EvalError::EmptyOperands);
    }
    let mut acc = (*inputs[0]).clone();
    for opd in &inputs[1..] {
        if acc.bw() != opd.bw() {
            return Err(EvalError::BitWidthMismatch(
                "or operands must have equal width".into(),
            ));
        }
        acc = acc.or(opd);
    }
    Ok(acc)
}

/// Evaluate `comb.xor`: Bitwise XOR across non-empty operands.
pub fn eval_xor(inputs: &[&APInt]) -> Result<APInt, EvalError> {
    if inputs.is_empty() {
        return Err(EvalError::EmptyOperands);
    }
    let mut acc = (*inputs[0]).clone();
    for opd in &inputs[1..] {
        if acc.bw() != opd.bw() {
            return Err(EvalError::BitWidthMismatch(
                "xor operands must have equal width".into(),
            ));
        }
        acc = acc.xor(opd);
    }
    Ok(acc)
}

/// Evaluate `comb.not`: Bitwise inversion `~val`.
pub fn eval_not(val: &APInt) -> APInt {
    let w = NonZero::new(val.bw()).expect("non-zero width");
    let all_ones = APInt::umax(w);
    val.xor(&all_ones)
}

/// Evaluate `comb.neg`: Two's-complement negation `-val = 0 - val`.
pub fn eval_neg(val: &APInt) -> APInt {
    let w = NonZero::new(val.bw()).expect("non-zero width");
    let zero = APInt::zero(w);
    zero.sub(val)
}

/// Evaluate `comb.any`: OR reduction. Returns 1-bit `1` if any bit is set, else `0`.
pub fn eval_any(val: &APInt) -> APInt {
    let one_bit = NonZero::new(1).unwrap();
    if val.is_zero() {
        APInt::zero(one_bit)
    } else {
        APInt::uone(one_bit)
    }
}

/// Evaluate `comb.all`: AND reduction. Returns 1-bit `1` if all bits are set, else `0`.
pub fn eval_all(val: &APInt) -> APInt {
    let one_bit = NonZero::new(1).unwrap();
    let w = NonZero::new(val.bw()).expect("non-zero width");
    let all_ones = APInt::umax(w);
    if val == &all_ones {
        APInt::uone(one_bit)
    } else {
        APInt::zero(one_bit)
    }
}

/// Evaluate `comb.parity`: Parity reduction (XOR of all bits).
/// Returns 1-bit `1` if the number of set bits is odd, else `0`.
pub fn eval_parity(val: &APInt) -> APInt {
    let one_bit = NonZero::new(1).unwrap();
    // Count ones via awint bits
    let awi = &val.to_u128(); // for general widths, let's count directly via bit tests
    let mut ones = 0usize;
    for i in 0..val.bw() {
        let mask = APInt::uone(NonZero::new(val.bw()).unwrap())
            .shl(&APInt::from_usize(i, NonZero::new(val.bw()).unwrap()));
        if !val.and(&mask).is_zero() {
            ones += 1;
        }
    }
    if ones % 2 == 1 {
        APInt::uone(one_bit)
    } else {
        APInt::zero(one_bit)
    }
}

/// Evaluate `comb.icmp`: Integer comparison according to predicate.
pub fn eval_icmp(pred: ICmpPredicate, lhs: &APInt, rhs: &APInt) -> Result<APInt, EvalError> {
    if lhs.bw() != rhs.bw() {
        return Err(EvalError::BitWidthMismatch(
            "icmp operands must have equal width".into(),
        ));
    }
    let one_bit = NonZero::new(1).unwrap();
    let b = match pred {
        ICmpPredicate::EQ => lhs == rhs,
        ICmpPredicate::NE => lhs != rhs,
        ICmpPredicate::SLT => lhs.slt(rhs),
        ICmpPredicate::SLE => lhs.sle(rhs),
        ICmpPredicate::SGT => lhs.sgt(rhs),
        ICmpPredicate::SGE => lhs.sge(rhs),
        ICmpPredicate::ULT => lhs.ult(rhs),
        ICmpPredicate::ULE => lhs.ule(rhs),
        ICmpPredicate::UGT => lhs.ugt(rhs),
        ICmpPredicate::UGE => lhs.uge(rhs),
    };
    if b {
        Ok(APInt::uone(one_bit))
    } else {
        Ok(APInt::zero(one_bit))
    }
}

/// Evaluate `comb.mux`: 2-to-1 Multiplexer.
pub fn eval_mux(cond: &APInt, true_val: &APInt, false_val: &APInt) -> Result<APInt, EvalError> {
    if cond.bw() != 1 {
        return Err(EvalError::BitWidthMismatch(
            "mux condition must be 1-bit".into(),
        ));
    }
    if true_val.bw() != false_val.bw() {
        return Err(EvalError::BitWidthMismatch(
            "mux branch values must have equal width".into(),
        ));
    }
    if !cond.is_zero() {
        Ok(true_val.clone())
    } else {
        Ok(false_val.clone())
    }
}

/// Evaluate `comb.concat`: Concatenates operands from high bits to low bits.
///
/// Input `[v_hi, v_lo]` yields a word where `v_hi` occupies the MSBs and `v_lo` the LSBs.
pub fn eval_concat(inputs: &[&APInt]) -> Result<APInt, EvalError> {
    if inputs.is_empty() {
        return Err(EvalError::EmptyOperands);
    }
    let total_width: usize = inputs.iter().map(|x| x.bw()).sum();
    let width_nz = NonZero::new(total_width).ok_or(EvalError::ZeroWidth)?;
    let mut res = Awi::zero(width_nz);

    let mut shift_offset = total_width;
    for opd in inputs {
        shift_offset -= opd.bw();
        for b in 0..opd.bw() {
            let mask = APInt::uone(NonZero::new(opd.bw()).unwrap())
                .shl(&APInt::from_usize(b, NonZero::new(opd.bw()).unwrap()));
            let bit_val = !opd.and(&mask).is_zero();
            res.set(shift_offset + b, bit_val);
        }
    }
    Ok(
        APInt::from_str(&res.to_string(), total_width, 10)
            .unwrap_or_else(|_| APInt::zero(width_nz)),
    )
}

/// Evaluate `comb.extract`: Extract a bit slice `[low_bit + width - 1 : low_bit]`.
pub fn eval_extract(val: &APInt, low_bit: usize, width: usize) -> Result<APInt, EvalError> {
    if width == 0 {
        return Err(EvalError::ZeroWidth);
    }
    if low_bit + width > val.bw() {
        return Err(EvalError::OutOfBounds(format!(
            "low_bit ({}) + width ({}) exceeds input width ({})",
            low_bit,
            width,
            val.bw()
        )));
    }
    let width_nz = NonZero::new(width).unwrap();
    let mut res = Awi::zero(width_nz);
    for i in 0..width {
        let mask = APInt::uone(NonZero::new(val.bw()).unwrap()).shl(&APInt::from_usize(
            low_bit + i,
            NonZero::new(val.bw()).unwrap(),
        ));
        let bit_val = !val.and(&mask).is_zero();
        res.set(i, bit_val);
    }
    Ok(APInt::from_str(&res.to_string(), width, 10).unwrap_or_else(|_| APInt::zero(width_nz)))
}

/// Evaluate `comb.replicate`: Replicate input `count` times.
pub fn eval_replicate(val: &APInt, count: usize) -> Result<APInt, EvalError> {
    if count == 0 {
        return Err(EvalError::ZeroWidth);
    }
    let total_width = val.bw() * count;
    let width_nz = NonZero::new(total_width).ok_or(EvalError::ZeroWidth)?;
    let mut res = Awi::zero(width_nz);
    for k in 0..count {
        for i in 0..val.bw() {
            let mask = APInt::uone(NonZero::new(val.bw()).unwrap())
                .shl(&APInt::from_usize(i, NonZero::new(val.bw()).unwrap()));
            let bit_val = !val.and(&mask).is_zero();
            res.set(k * val.bw() + i, bit_val);
        }
    }
    Ok(
        APInt::from_str(&res.to_string(), total_width, 10)
            .unwrap_or_else(|_| APInt::zero(width_nz)),
    )
}

// -----------------------------------------------------------------------------
// Constant Folder for comb Operations
// -----------------------------------------------------------------------------

/// Attempt to retrieve the constant `APInt` value of an SSA value, if produced by `hw.constant`.
pub fn get_constant_value(ctx: &Context, val: Value) -> Option<APInt> {
    let def_op = val.defining_op()?;
    let const_op = Operation::get_op::<ConstantOp>(def_op, ctx)?;
    Some(const_op.get_attr_value(ctx)?.value().clone())
}

/// Attempt to constant fold a `comb` operation if all of its operands are known constants.
///
/// Returns `Ok(Some(result_apint))` if folded successfully.
/// Returns `Ok(None)` if the operation has non-constant inputs or division/remainder by zero (undefined).
pub fn fold_comb_op(ctx: &Context, op_ptr: Ptr<Operation>) -> Result<Option<APInt>, EvalError> {
    let op = op_ptr.deref(ctx);
    let op_id = Operation::get_opid(op_ptr, ctx).name;
    let name: &str = op_id.as_ref();

    // All comb ops require constant operands to fold
    let mut opd_values = Vec::with_capacity(op.get_num_operands());
    for opd in op.operands() {
        match get_constant_value(ctx, opd) {
            Some(v) => opd_values.push(v),
            None => return Ok(None),
        }
    }

    match name {
        "comb.add" => {
            let r = eval_add(&opd_values[0], &opd_values[1])?;
            Ok(Some(r))
        }
        "comb.sub" => {
            let r = eval_sub(&opd_values[0], &opd_values[1])?;
            Ok(Some(r))
        }
        "comb.mul" => {
            let r = eval_mul(&opd_values[0], &opd_values[1])?;
            Ok(Some(r))
        }
        "comb.divu" => {
            match eval_divu(&opd_values[0], &opd_values[1]) {
                Ok(r) => Ok(Some(r)),
                Err(EvalError::DivisionByZero) => Ok(None), // Division by zero is undefined; do not fold
                Err(e) => Err(e),
            }
        }
        "comb.divs" => match eval_divs(&opd_values[0], &opd_values[1]) {
            Ok(r) => Ok(Some(r)),
            Err(EvalError::DivisionByZero) => Ok(None),
            Err(e) => Err(e),
        },
        "comb.modu" => match eval_modu(&opd_values[0], &opd_values[1]) {
            Ok(r) => Ok(Some(r)),
            Err(EvalError::RemainderByZero) => Ok(None),
            Err(e) => Err(e),
        },
        "comb.mods" => match eval_mods(&opd_values[0], &opd_values[1]) {
            Ok(r) => Ok(Some(r)),
            Err(EvalError::RemainderByZero) => Ok(None),
            Err(e) => Err(e),
        },
        "comb.shl" => {
            let r = eval_shl(&opd_values[0], &opd_values[1])?;
            Ok(Some(r))
        }
        "comb.shru" => {
            let r = eval_shru(&opd_values[0], &opd_values[1])?;
            Ok(Some(r))
        }
        "comb.shrs" => {
            let r = eval_shrs(&opd_values[0], &opd_values[1])?;
            Ok(Some(r))
        }
        "comb.and" => {
            let refs: Vec<&APInt> = opd_values.iter().collect();
            let r = eval_and(&refs)?;
            Ok(Some(r))
        }
        "comb.or" => {
            let refs: Vec<&APInt> = opd_values.iter().collect();
            let r = eval_or(&refs)?;
            Ok(Some(r))
        }
        "comb.xor" => {
            let refs: Vec<&APInt> = opd_values.iter().collect();
            let r = eval_xor(&refs)?;
            Ok(Some(r))
        }
        "comb.not" => {
            let r = eval_not(&opd_values[0]);
            Ok(Some(r))
        }
        "comb.neg" => {
            let r = eval_neg(&opd_values[0]);
            Ok(Some(r))
        }
        "comb.any" => {
            let r = eval_any(&opd_values[0]);
            Ok(Some(r))
        }
        "comb.all" => {
            let r = eval_all(&opd_values[0]);
            Ok(Some(r))
        }
        "comb.parity" => {
            let r = eval_parity(&opd_values[0]);
            Ok(Some(r))
        }
        "comb.mux" => {
            let r = eval_mux(&opd_values[0], &opd_values[1], &opd_values[2])?;
            Ok(Some(r))
        }
        "comb.icmp" => {
            let icmp_op = Operation::get_op::<ICmpOp>(op_ptr, ctx).expect("icmp op");
            let pred = icmp_op.predicate(ctx);
            let r = eval_icmp(pred, &opd_values[0], &opd_values[1])?;
            Ok(Some(r))
        }
        "comb.concat" => {
            let refs: Vec<&APInt> = opd_values.iter().collect();
            let r = eval_concat(&refs)?;
            Ok(Some(r))
        }
        "comb.extract" => {
            let ext_op = Operation::get_op::<ExtractOp>(op_ptr, ctx).expect("extract op");
            let low_bit = ext_op.low_bit(ctx).value().to_u64() as usize;
            let res_w = ext_op
                .result(ctx)
                .get_type(ctx)
                .deref(ctx)
                .downcast_ref::<IntegerType>()
                .unwrap()
                .width() as usize;
            let r = eval_extract(&opd_values[0], low_bit, res_w)?;
            Ok(Some(r))
        }
        "comb.replicate" => {
            let rep_op = Operation::get_op::<ReplicateOp>(op_ptr, ctx).expect("replicate op");
            let count = rep_op.count(ctx).value().to_u64() as usize;
            let r = eval_replicate(&opd_values[0], count)?;
            Ok(Some(r))
        }
        _ => Err(EvalError::UnsupportedOperation(name.to_string())),
    }
}

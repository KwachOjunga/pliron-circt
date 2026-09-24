// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

use awint::bw;
use pliron::{
    basic_block::BasicBlock,
    builtin::{
        attributes::IntegerAttr,
        types::{IntegerType, Signedness},
    },
    context::{Context, Ptr},
    op::{Op, verify_op},
    r#type::{TypeHandle, Typed},
    utils::apint::APInt,
};
use pliron_circt::{
    comb::ops::{
        AddOp, AllOp, AndOp, AnyOp, ConcatOp, DivUOp, ExtractOp, ICmpOp, ICmpPredicate, ModUOp,
        MulOp, MuxOp, NegOp, NotOp, OrOp, ParityOp, ReplicateOp, ShlOp, ShrSOp, ShrUOp, SubOp,
        XorOp,
    },
    hw::ops::{ConstantOp, ModuleOp, OutputOp},
    register_all,
};

fn int_attr(ctx: &mut Context, width: u32, val: u64) -> IntegerAttr {
    let ty = IntegerType::get(ctx, width, Signedness::Signless);
    IntegerAttr::new(ty, APInt::from_u64(val, bw(width as usize)))
}

fn create_test_module(ctx: &mut Context, name: &str) -> (ModuleOp, Ptr<BasicBlock>) {
    let mod_name = name.try_into().unwrap();
    let module = ModuleOp::new(ctx, mod_name, vec![]);
    let body = module.get_body(ctx);
    (module, body)
}

#[test]
// Covers construction and verification of arithmetic dataflow operations.
fn test_comb_arithmetic_ops() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let (module, body) = create_test_module(&mut ctx, "arith_test");
    let i32_ty: TypeHandle = IntegerType::get(&ctx, 32, Signedness::Signless).into();

    let a10 = int_attr(&mut ctx, 32, 10);
    let c10 = ConstantOp::new(&mut ctx, a10);
    let a20 = int_attr(&mut ctx, 32, 20);
    let c20 = ConstantOp::new(&mut ctx, a20);

    c10.get_operation().insert_at_back(body, &ctx);
    c20.get_operation().insert_at_back(body, &ctx);

    let v10 = c10.result(&ctx);
    let v20 = c20.result(&ctx);

    let add = AddOp::new(&mut ctx, vec![v10, v20], i32_ty);
    assert_eq!(add.result(&ctx).get_type(&ctx), i32_ty);
    add.get_operation().insert_at_back(body, &ctx);

    let sub = SubOp::new(&mut ctx, v20, v10, i32_ty);
    assert_eq!(sub.result(&ctx).get_type(&ctx), i32_ty);
    sub.get_operation().insert_at_back(body, &ctx);

    let mul = MulOp::new(&mut ctx, vec![v10, v20], i32_ty);
    assert_eq!(mul.result(&ctx).get_type(&ctx), i32_ty);
    mul.get_operation().insert_at_back(body, &ctx);

    let div = DivUOp::new(&mut ctx, v20, v10, i32_ty);
    assert_eq!(div.result(&ctx).get_type(&ctx), i32_ty);
    div.get_operation().insert_at_back(body, &ctx);

    let rem = ModUOp::new(&mut ctx, v20, v10, i32_ty);
    assert_eq!(rem.result(&ctx).get_type(&ctx), i32_ty);
    rem.get_operation().insert_at_back(body, &ctx);

    let shl = ShlOp::new(&mut ctx, v10, v20, i32_ty);
    assert_eq!(shl.result(&ctx).get_type(&ctx), i32_ty);
    shl.get_operation().insert_at_back(body, &ctx);

    let shru = ShrUOp::new(&mut ctx, v20, v10, i32_ty);
    assert_eq!(shru.result(&ctx).get_type(&ctx), i32_ty);
    shru.get_operation().insert_at_back(body, &ctx);

    let shrs = ShrSOp::new(&mut ctx, v20, v10, i32_ty);
    assert_eq!(shrs.result(&ctx).get_type(&ctx), i32_ty);
    shrs.get_operation().insert_at_back(body, &ctx);

    let add_res = add.result(&ctx);
    let out = OutputOp::new(&mut ctx, vec![add_res]);
    out.get_operation().insert_at_back(body, &ctx);

    verify_op(&module, &ctx).expect("comb arithmetic in module should verify");
}

#[test]
// Covers variadic bitwise logic, comparisons, and conditional selection.
fn test_comb_logical_and_selection_ops() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let (module, body) = create_test_module(&mut ctx, "logic_test");
    let i8_ty: TypeHandle = IntegerType::get(&ctx, 8, Signedness::Signless).into();
    let i1_ty: TypeHandle = IntegerType::get(&ctx, 1, Signedness::Signless).into();

    let a_val = int_attr(&mut ctx, 8, 0b10101010);
    let c_a = ConstantOp::new(&mut ctx, a_val);
    let b_val = int_attr(&mut ctx, 8, 0b11001100);
    let c_b = ConstantOp::new(&mut ctx, b_val);
    let c_val = int_attr(&mut ctx, 8, 0b11110000);
    let c_c = ConstantOp::new(&mut ctx, c_val);

    c_a.get_operation().insert_at_back(body, &ctx);
    c_b.get_operation().insert_at_back(body, &ctx);
    c_c.get_operation().insert_at_back(body, &ctx);

    let va = c_a.result(&ctx);
    let vb = c_b.result(&ctx);
    let vc = c_c.result(&ctx);

    let and_op = AndOp::new(&mut ctx, vec![va, vb, vc], i8_ty);
    assert_eq!(and_op.result(&ctx).get_type(&ctx), i8_ty);
    and_op.get_operation().insert_at_back(body, &ctx);

    let or_op = OrOp::new(&mut ctx, vec![va, vb], i8_ty);
    assert_eq!(or_op.result(&ctx).get_type(&ctx), i8_ty);
    or_op.get_operation().insert_at_back(body, &ctx);

    let xor_op = XorOp::new(&mut ctx, vec![va, vb], i8_ty);
    assert_eq!(xor_op.result(&ctx).get_type(&ctx), i8_ty);
    xor_op.get_operation().insert_at_back(body, &ctx);

    // ICmp
    let cmp_eq = ICmpOp::new(&mut ctx, ICmpPredicate::EQ, va, vb, i1_ty);
    assert_eq!(cmp_eq.result(&ctx).get_type(&ctx), i1_ty);
    cmp_eq.get_operation().insert_at_back(body, &ctx);

    let cmp_slt = ICmpOp::new(&mut ctx, ICmpPredicate::SLT, va, vb, i1_ty);
    assert_eq!(cmp_slt.result(&ctx).get_type(&ctx), i1_ty);
    cmp_slt.get_operation().insert_at_back(body, &ctx);

    let cond = cmp_eq.result(&ctx);
    let mux_op = MuxOp::new(&mut ctx, cond, va, vb, i8_ty);
    assert_eq!(mux_op.result(&ctx).get_type(&ctx), i8_ty);
    mux_op.get_operation().insert_at_back(body, &ctx);

    let mux_op_res = mux_op.result(&ctx);
    let out = OutputOp::new(&mut ctx, vec![mux_op_res]);
    out.get_operation().insert_at_back(body, &ctx);

    verify_op(&module, &ctx).expect("comb logical and selection in module should verify");
}

#[test]
// Covers concatenation, extraction, replication, and parity reductions.
fn test_comb_bit_manipulations() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let (module, body) = create_test_module(&mut ctx, "bit_test");
    let i16_ty: TypeHandle = IntegerType::get(&ctx, 16, Signedness::Signless).into();
    let i4_ty: TypeHandle = IntegerType::get(&ctx, 4, Signedness::Signless).into();
    let i32_ty: TypeHandle = IntegerType::get(&ctx, 32, Signedness::Signless).into();
    let i1_ty: TypeHandle = IntegerType::get(&ctx, 1, Signedness::Signless).into();

    let a_hi = int_attr(&mut ctx, 8, 0xAB);
    let c_hi = ConstantOp::new(&mut ctx, a_hi);
    let a_lo = int_attr(&mut ctx, 8, 0xCD);
    let c_lo = ConstantOp::new(&mut ctx, a_lo);

    c_hi.get_operation().insert_at_back(body, &ctx);
    c_lo.get_operation().insert_at_back(body, &ctx);

    let v_hi = c_hi.result(&ctx);
    let v_lo = c_lo.result(&ctx);

    let concat = ConcatOp::new(&mut ctx, vec![v_hi, v_lo], i16_ty);
    assert_eq!(concat.result(&ctx).get_type(&ctx), i16_ty);
    concat.get_operation().insert_at_back(body, &ctx);

    let low_idx = int_attr(&mut ctx, 32, 4);
    let extract = ExtractOp::new(&mut ctx, v_hi, low_idx, i4_ty);
    assert_eq!(extract.result(&ctx).get_type(&ctx), i4_ty);
    extract.get_operation().insert_at_back(body, &ctx);

    let rep_count = int_attr(&mut ctx, 32, 4);
    let replicate = ReplicateOp::new(&mut ctx, v_hi, rep_count, i32_ty);
    assert_eq!(replicate.result(&ctx).get_type(&ctx), i32_ty);
    replicate.get_operation().insert_at_back(body, &ctx);

    let parity = ParityOp::new(&mut ctx, v_hi, i1_ty);
    assert_eq!(parity.result(&ctx).get_type(&ctx), i1_ty);
    parity.get_operation().insert_at_back(body, &ctx);

    let concat_res = concat.result(&ctx);
    let out = OutputOp::new(&mut ctx, vec![concat_res]);
    out.get_operation().insert_at_back(body, &ctx);

    verify_op(&module, &ctx).expect("comb bit manipulations in module should verify");
}

#[test]
// Covers unary bitwise/negation operations and any/all reductions.
fn test_comb_unary_and_reduction_ops() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let (module, body) = create_test_module(&mut ctx, "unary_test");
    let i8_ty: TypeHandle = IntegerType::get(&ctx, 8, Signedness::Signless).into();
    let i1_ty: TypeHandle = IntegerType::get(&ctx, 1, Signedness::Signless).into();
    let constant_attr = int_attr(&mut ctx, 8, 0x55);
    let constant = ConstantOp::new(&mut ctx, constant_attr);
    constant.get_operation().insert_at_back(body, &ctx);

    let value = constant.result(&ctx);
    let not = NotOp::new(&mut ctx, value, i8_ty);
    let neg = NegOp::new(&mut ctx, value, i8_ty);
    let any = AnyOp::new(&mut ctx, value, i1_ty);
    let all = AllOp::new(&mut ctx, value, i1_ty);
    for op in [
        not.get_operation(),
        neg.get_operation(),
        any.get_operation(),
        all.get_operation(),
    ] {
        op.insert_at_back(body, &ctx);
    }

    let not_result = not.result(&ctx);
    let output = OutputOp::new(&mut ctx, vec![not_result]);
    output.get_operation().insert_at_back(body, &ctx);
    verify_op(&module, &ctx).expect("comb unary and reduction ops should verify");
}

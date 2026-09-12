// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Tests ported and adapted from xDSL hardware dialects (`test_hw.py`, `comb_ops.mlir`, `seq_ops.mlir`).
//!
//! These tests verify semantic contracts established by xDSL and CIRCT parity targets:
//! - Module creation and port validation
//! - Array creation and indexing
//! - Instance argument and result types
//! - Sequential clock and register behavior
//! - Combinational logic, comparison predicates, and bit slicing

use awint::bw;
use pliron::{
    basic_block::BasicBlock,
    builtin::{
        attributes::{IntegerAttr, StringAttr},
        types::{IntegerType, Signedness},
    },
    context::{Context, Ptr},
    identifier::Identifier,
    op::{Op, verify_op},
    r#type::{TypeHandle, Typed},
    utils::apint::APInt,
};
use pliron_hw::{
    comb::ops::{AddOp, AndOp, ExtractOp, ICmpOp, ICmpPredicate},
    hw::{
        ops::{ArrayCreateOp, ArrayGetOp, ConstantOp, InstanceOp, ModuleOp, OutputOp},
        types::ArrayType,
    },
    register_all,
    seq::{
        ops::{ClockGateOp, CompRegOp, FirRegOp},
        types::{ClockType, ResetType},
    },
};

fn int_attr(ctx: &mut Context, width: u32, val: u64) -> IntegerAttr {
    let ty = IntegerType::get(ctx, width, Signedness::Signless);
    IntegerAttr::new(ty, APInt::from_u64(val, bw(width as usize)))
}

fn create_test_module(ctx: &mut Context, name: &str, inputs: Vec<TypeHandle>) -> (ModuleOp, Ptr<BasicBlock>) {
    let mod_name: Identifier = name.try_into().unwrap();
    let module = ModuleOp::new(ctx, mod_name, inputs);
    let body = module.get_body(ctx);
    (module, body)
}

#[test]
// Ported from xDSL `test_hw.py::test_instance_builder`
// Verifies module instantiation with input arguments, output results, and port parity.
fn test_xdsl_instance_port_parity() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let i32_ty: TypeHandle = IntegerType::get(&mut ctx, 32, Signedness::Signless).into();
    let i64_ty: TypeHandle = IntegerType::get(&mut ctx, 64, Signedness::Signless).into();

    // hw.module @target_module(in %foo: i32, in %bar: i64, out baz: i32, out qux: i64)
    let (target_mod, body) = create_test_module(&mut ctx, "target_module", vec![i32_ty, i64_ty]);

    let foo = target_mod.get_input(&ctx, 0);
    let bar = target_mod.get_input(&ctx, 1);
    let out_op = OutputOp::new(&mut ctx, vec![foo, bar]);
    out_op.get_operation().insert_at_back(body, &mut ctx);

    verify_op(&target_mod, &ctx).expect("target module must verify");

    // Instantiating the module
    let inst_name: StringAttr = "test_instance".to_string().into();
    let mod_id: Identifier = "target_module".try_into().unwrap();
    let instance = InstanceOp::new(
        &mut ctx,
        inst_name,
        mod_id.into(),
        vec![foo, bar],
        vec![i32_ty, i64_ty],
    );

    assert_eq!(instance.instance_name(&ctx).as_ref(), "test_instance");
    assert_eq!(instance.module_name(&ctx).as_ref().as_ref(), "target_module");
    let results = instance.results(&ctx);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].get_type(&ctx), i32_ty);
    assert_eq!(results[1].get_type(&ctx), i64_ty);

    verify_op(&instance, &ctx).expect("instance must verify");
}

#[test]
// Ported from xDSL `tests/filecheck/dialects/hw/hw_ops.mlir`
// Tests hw.array_create and hw.array_get with matching index width
fn test_xdsl_array_ops_roundtrip() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let (module, body) = create_test_module(&mut ctx, "array_mod", vec![]);

    let i19_ty: TypeHandle = IntegerType::get(&mut ctx, 19, Signedness::Signless).into();

    let c19_attr = int_attr(&mut ctx, 19, 42);
    let c19_op = ConstantOp::new(&mut ctx, c19_attr);
    let val = c19_op.result(&ctx);
    c19_op.get_operation().insert_at_back(body, &mut ctx);

    // hw.array_create %val, %val : i19 -> !hw.array<2xi19>
    let arr_ty = ArrayType::get(&mut ctx, 2, i19_ty).into();
    let create_op = ArrayCreateOp::new(&mut ctx, vec![val, val], arr_ty);
    let arr = create_op.result(&ctx);
    create_op.get_operation().insert_at_back(body, &mut ctx);

    // hw.array_get %arr[%idx] : !hw.array<2xi19>, i1
    let idx_attr = int_attr(&mut ctx, 1, 0);
    let idx_op = ConstantOp::new(&mut ctx, idx_attr);
    let idx = idx_op.result(&ctx);
    idx_op.get_operation().insert_at_back(body, &mut ctx);

    let get_op = ArrayGetOp::new(&mut ctx, arr, idx, i19_ty);
    get_op.get_operation().insert_at_back(body, &mut ctx);
    assert_eq!(get_op.result(&ctx).get_type(&ctx), i19_ty);

    let final_res = get_op.result(&ctx);
    let out_op = OutputOp::new(&mut ctx, vec![final_res]);
    out_op.get_operation().insert_at_back(body, &mut ctx);

    verify_op(&module, &ctx).expect("module with array ops should verify");
}

#[test]
// Ported from xDSL `tests/filecheck/dialects/comb/comb_ops.mlir`
// Tests comb.add, comb.and, comb.extract, and comb.icmp predicates
fn test_xdsl_comb_operations_parity() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let (module, body) = create_test_module(&mut ctx, "comb_mod", vec![]);

    let i32_ty: TypeHandle = IntegerType::get(&mut ctx, 32, Signedness::Signless).into();
    let i1_ty: TypeHandle = IntegerType::get(&mut ctx, 1, Signedness::Signless).into();

    let a_val = int_attr(&mut ctx, 32, 10);
    let b_val = int_attr(&mut ctx, 32, 20);
    let a_op = ConstantOp::new(&mut ctx, a_val);
    let b_op = ConstantOp::new(&mut ctx, b_val);

    let a = a_op.result(&ctx);
    let b = b_op.result(&ctx);
    a_op.get_operation().insert_at_back(body, &mut ctx);
    b_op.get_operation().insert_at_back(body, &mut ctx);

    // comb.add %a, %b : i32
    let add_op = AddOp::new(&mut ctx, a, b, i32_ty);
    assert_eq!(add_op.result(&ctx).get_type(&ctx), i32_ty);
    add_op.get_operation().insert_at_back(body, &mut ctx);

    // comb.and %a, %b : i32
    let and_op = AndOp::new(&mut ctx, vec![a, b], i32_ty);
    assert_eq!(and_op.result(&ctx).get_type(&ctx), i32_ty);
    and_op.get_operation().insert_at_back(body, &mut ctx);

    // comb.icmp eq %a, %b : i1
    let icmp_eq = ICmpOp::new(&mut ctx, ICmpPredicate::EQ, a, b, i1_ty);
    assert_eq!(icmp_eq.result(&ctx).get_type(&ctx), i1_ty);
    icmp_eq.get_operation().insert_at_back(body, &mut ctx);

    // comb.icmp slt %a, %b : i1
    let icmp_slt = ICmpOp::new(&mut ctx, ICmpPredicate::SLT, a, b, i1_ty);
    assert_eq!(icmp_slt.result(&ctx).get_type(&ctx), i1_ty);
    icmp_slt.get_operation().insert_at_back(body, &mut ctx);

    // comb.extract %a from 4 : (i32) -> i8
    let i8_ty: TypeHandle = IntegerType::get(&mut ctx, 8, Signedness::Signless).into();
    let low_bit = int_attr(&mut ctx, 32, 4);
    let extract_op = ExtractOp::new(&mut ctx, a, low_bit, i8_ty);
    assert_eq!(extract_op.result(&ctx).get_type(&ctx), i8_ty);
    extract_op.get_operation().insert_at_back(body, &mut ctx);

    let final_res = add_op.result(&ctx);
    let out_op = OutputOp::new(&mut ctx, vec![final_res]);
    out_op.get_operation().insert_at_back(body, &mut ctx);

    verify_op(&module, &ctx).expect("comb module should verify");
}

#[test]
// Ported from xDSL `tests/filecheck/dialects/seq/seq_ops.mlir`
// Tests seq.clock and seq.compreg / seq.firreg behavior
fn test_xdsl_seq_register_parity() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let clock_ty = ClockType::get(&mut ctx).into();
    let reset_ty = ResetType::get(&mut ctx).into();
    let i32_ty: TypeHandle = IntegerType::get(&mut ctx, 32, Signedness::Signless).into();
    let i1_ty: TypeHandle = IntegerType::get(&mut ctx, 1, Signedness::Signless).into();

    let (module, body) = create_test_module(
        &mut ctx,
        "seq_test_module",
        vec![clock_ty, i32_ty, reset_ty, i32_ty, i1_ty],
    );

    let clk = module.get_input(&ctx, 0);
    let d = module.get_input(&ctx, 1);
    let rst = module.get_input(&ctx, 2);
    let rst_val = module.get_input(&ctx, 3);
    let en = module.get_input(&ctx, 4);

    // seq.compreg %clk, %d : i32
    let reg_plain = CompRegOp::new(&mut ctx, clk, d, i32_ty);
    assert_eq!(reg_plain.result(&ctx).get_type(&ctx), i32_ty);
    reg_plain.get_operation().insert_at_back(body, &mut ctx);

    // seq.firreg %clk, %d reset %rst, %rst_val : i32
    let reg_with_reset = FirRegOp::new(
        &mut ctx,
        clk,
        d,
        rst,
        rst_val,
        i32_ty,
        false,
        "active_high",
    );
    assert_eq!(reg_with_reset.result(&ctx).get_type(&ctx), i32_ty);
    reg_with_reset.get_operation().insert_at_back(body, &mut ctx);

    // seq.clock_gate %clk, %en
    let clk_gate = ClockGateOp::new(&mut ctx, clk, en, clock_ty);
    assert_eq!(clk_gate.result(&ctx).get_type(&ctx), clock_ty);
    clk_gate.get_operation().insert_at_back(body, &mut ctx);

    let final_res = reg_plain.result(&ctx);
    let out_op = OutputOp::new(&mut ctx, vec![final_res]);
    out_op.get_operation().insert_at_back(body, &mut ctx);

    verify_op(&module, &ctx).expect("seq module should verify");
}

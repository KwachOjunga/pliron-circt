// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

use awint::bw;
use pliron::{
    builtin::{
        attributes::IntegerAttr,
        op_interfaces::RegionKind,
    },
    common_traits::Named,
    context::Context,
    identifier::Identifier,
    op::{Op, verify_op},
    printable::Printable,
    r#type::TypeHandle,
    utils::apint::APInt,
};
use pliron_hw::{
    hw::{
        ops::{
            ArrayCreateOp, ArrayGetOp, BitcastOp, ConcatOp, ConstantOp, ExternModuleOp,
            InstanceOp, ModuleOp, OutputOp, SliceOp, StructCreateOp, StructExplodeOp,
            StructExtractOp, StructInjectOp, WireOp,
        },
        types::{ArrayType, IntType, StructType},
    },
    register_all,
};

#[test]
fn test_hw_module_creation_and_graph_region() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let i1: TypeHandle = IntType::get(&mut ctx, 1).into();
    let i32: TypeHandle = IntType::get(&mut ctx, 32).into();

    let module_name = "alu".try_into().unwrap();
    let module = ModuleOp::new(&mut ctx, module_name, vec![i32, i32, i1]);

    // Check module properties
    assert_eq!(module.num_inputs(&ctx), 3);
    assert_eq!(module.get_input(&ctx, 0).get_type(&ctx), i32);
    assert_eq!(module.get_input(&ctx, 2).get_type(&ctx), i1);

    // Verify RegionKind is Graph and has_ssa_dominance is false
    let region = module.get_region(&ctx);
    assert!(!region.deref(&ctx).has_ssa_dominance(&ctx));

    // Create a constant and an output
    let const_val = IntegerAttr::new(i32, APInt::from_u64(42, bw(32)));
    let const_op = ConstantOp::new(&mut ctx, const_val);
    let output_op = OutputOp::new(&mut ctx, vec![const_op.result(&ctx)]);

    let body = module.get_body(&ctx);
    const_op.get_operation().insert_at_back(body, &mut ctx);
    output_op.get_operation().insert_at_back(body, &mut ctx);

    // Verify the entire module IR
    verify_op(&module, &ctx).expect("hw.module should verify successfully");

    // Print representation
    let ir_str = module.get_operation().disp(&ctx).to_string();
    assert!(ir_str.contains("hw.module"));
    assert!(ir_str.contains("@alu"));
}

#[test]
fn test_hw_wire_and_bitcast() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let i8: TypeHandle = IntType::get(&mut ctx, 8).into();
    let const_val = IntegerAttr::new(i8, APInt::from_u64(0x55, bw(8)));
    let const_op = ConstantOp::new(&mut ctx, const_val);

    let wire_name = "data_bus".to_string().into();
    let wire_op = WireOp::new(&mut ctx, wire_name, const_op.result(&ctx));
    assert_eq!(wire_op.result(&ctx).get_type(&ctx), i8);

    let bitcast_op = BitcastOp::new(&mut ctx, wire_op.result(&ctx), i8);
    assert_eq!(bitcast_op.result(&ctx).get_type(&ctx), i8);

    verify_op(&wire_op, &ctx).expect("hw.wire should verify");
    verify_op(&bitcast_op, &ctx).expect("hw.bitcast should verify");
}

#[test]
fn test_hw_concat_and_slice() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let i8: TypeHandle = IntType::get(&mut ctx, 8).into();
    let i16: TypeHandle = IntType::get(&mut ctx, 16).into();

    let c1 = ConstantOp::new(&mut ctx, IntegerAttr::new(i8, APInt::from_u64(0xAB, bw(8))));
    let c2 = ConstantOp::new(&mut ctx, IntegerAttr::new(i8, APInt::from_u64(0xCD, bw(8))));

    let concat_op = ConcatOp::new(&mut ctx, vec![c1.result(&ctx), c2.result(&ctx)], i16);
    assert_eq!(concat_op.result(&ctx).get_type(&ctx), i16);

    let low_bit = IntegerAttr::new(IntType::get(&mut ctx, 32).into(), APInt::from_u64(4, bw(32)));
    let slice_op = SliceOp::new(&mut ctx, concat_op.result(&ctx), low_bit, i8);
    assert_eq!(slice_op.result(&ctx).get_type(&ctx), i8);

    verify_op(&concat_op, &ctx).expect("hw.concat should verify");
    verify_op(&slice_op, &ctx).expect("hw.slice should verify");
}

#[test]
fn test_hw_array_operations() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let i8: TypeHandle = IntType::get(&mut ctx, 8).into();
    let i2: TypeHandle = IntType::get(&mut ctx, 2).into();
    let arr_ty: TypeHandle = ArrayType::get(&mut ctx, 4, i8).into();

    let e0 = ConstantOp::new(&mut ctx, IntegerAttr::new(i8, APInt::from_u64(10, bw(8))));
    let e1 = ConstantOp::new(&mut ctx, IntegerAttr::new(i8, APInt::from_u64(20, bw(8))));
    let e2 = ConstantOp::new(&mut ctx, IntegerAttr::new(i8, APInt::from_u64(30, bw(8))));
    let e3 = ConstantOp::new(&mut ctx, IntegerAttr::new(i8, APInt::from_u64(40, bw(8))));

    let arr_create = ArrayCreateOp::new(
        &mut ctx,
        vec![e0.result(&ctx), e1.result(&ctx), e2.result(&ctx), e3.result(&ctx)],
        arr_ty,
    );
    assert_eq!(arr_create.result(&ctx).get_type(&ctx), arr_ty);

    let idx = ConstantOp::new(&mut ctx, IntegerAttr::new(i2, APInt::from_u64(2, bw(2))));
    let arr_get = ArrayGetOp::new(&mut ctx, arr_create.result(&ctx), idx.result(&ctx), i8);
    assert_eq!(arr_get.result(&ctx).get_type(&ctx), i8);

    verify_op(&arr_create, &ctx).expect("hw.array_create should verify");
    verify_op(&arr_get, &ctx).expect("hw.array_get should verify");
}

#[test]
fn test_hw_struct_operations() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let i8: TypeHandle = IntType::get(&mut ctx, 8).into();
    let i16: TypeHandle = IntType::get(&mut ctx, 16).into();

    let f_a: Identifier = "a".try_into().unwrap();
    let f_b: Identifier = "b".try_into().unwrap();

    let struct_ty: TypeHandle = StructType::get(&mut ctx, vec![(f_a.clone(), i8), (f_b.clone(), i16)]).into();

    let val_a = ConstantOp::new(&mut ctx, IntegerAttr::new(i8, APInt::from_u64(1, bw(8))));
    let val_b = ConstantOp::new(&mut ctx, IntegerAttr::new(i16, APInt::from_u64(2, bw(16))));

    let struct_create = StructCreateOp::new(&mut ctx, vec![val_a.result(&ctx), val_b.result(&ctx)], struct_ty);
    assert_eq!(struct_create.result(&ctx).get_type(&ctx), struct_ty);

    let extract_a = StructExtractOp::new(&mut ctx, struct_create.result(&ctx), "a".to_string().into(), i8);
    assert_eq!(extract_a.result(&ctx).get_type(&ctx), i8);

    let new_val_a = ConstantOp::new(&mut ctx, IntegerAttr::new(i8, APInt::from_u64(99, bw(8))));
    let inject_a = StructInjectOp::new(&mut ctx, struct_create.result(&ctx), "a".to_string().into(), new_val_a.result(&ctx), struct_ty);
    assert_eq!(inject_a.result(&ctx).get_type(&ctx), struct_ty);

    let explode = StructExplodeOp::new(&mut ctx, struct_create.result(&ctx), vec![i8, i16]);
    assert_eq!(explode.results(&ctx).len(), 2);

    verify_op(&struct_create, &ctx).expect("hw.struct_create should verify");
    verify_op(&extract_a, &ctx).expect("hw.struct_extract should verify");
    verify_op(&inject_a, &ctx).expect("hw.struct_inject should verify");
    verify_op(&explode, &ctx).expect("hw.struct_explode should verify");
}

#[test]
fn test_hw_extern_module_and_instance() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let ext_name: Identifier = "pll_clk_gen".try_into().unwrap();
    let extern_mod = ExternModuleOp::new(&mut ctx, ext_name.clone());
    verify_op(&extern_mod, &ctx).expect("hw.module.extern should verify");

    let i1: TypeHandle = IntType::get(&mut ctx, 1).into();
    let clk_in = ConstantOp::new(&mut ctx, IntegerAttr::new(i1, APInt::from_u64(1, bw(1))));

    let inst = InstanceOp::new(
        &mut ctx,
        "u_pll".to_string().into(),
        ext_name.into(),
        vec![clk_in.result(&ctx)],
        vec![i1],
    );
    assert_eq!(inst.results(&ctx).len(), 1);
    verify_op(&inst, &ctx).expect("hw.instance should verify");
}


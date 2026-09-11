// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

use awint::bw;
use pliron::{
    basic_block::BasicBlock,
    builtin::{
        attributes::IntegerAttr,
        op_interfaces::OneRegionInterface,
        types::{IntegerType, Signedness},
    },
    context::{Context, Ptr},
    identifier::Identifier,
    op::{Op, verify_op},
    printable::Printable,
    r#type::{TypeHandle, Typed},
    utils::apint::APInt,
};
use pliron_hw::{
    hw::{
        ops::{
            ArrayCreateOp, ArrayGetOp, ArrayInjectOp, BitcastOp, ConcatOp, ConstantOp,
            ExternModuleOp, HierPathOp, InstanceOp, ModuleOp, OutputOp, ParamDeclOp,
            ParamValueOp, SliceOp, StructCreateOp, StructExplodeOp, StructExtractOp,
            StructInjectOp, UnionCreateOp, UnionExtractOp, WireOp,
        },
        types::{
            ArrayType, EnumType, EnumVariant, InoutType, IntType, StructField, StructType,
            TypeAliasType, UnionType,
        },
    },
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
fn test_hw_module_creation_and_graph_region() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let i1: TypeHandle = IntegerType::get(&mut ctx, 1, Signedness::Signless).into();
    let i32: TypeHandle = IntegerType::get(&mut ctx, 32, Signedness::Signless).into();

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
    let const_val = int_attr(&mut ctx, 32, 42);
    let const_op = ConstantOp::new(&mut ctx, const_val);
    let c_res = const_op.result(&ctx);
    let output_op = OutputOp::new(&mut ctx, vec![c_res]);

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

    let (module, body) = create_test_module(&mut ctx, "wire_test");

    let i8: TypeHandle = IntegerType::get(&mut ctx, 8, Signedness::Signless).into();
    let const_val = int_attr(&mut ctx, 8, 0x55);
    let const_op = ConstantOp::new(&mut ctx, const_val);

    let c_res = const_op.result(&ctx);
    let wire_name = "data_bus".to_string().into();
    let wire_op = WireOp::new(&mut ctx, wire_name, c_res);
    assert_eq!(wire_op.result(&ctx).get_type(&ctx), i8);

    let w_res = wire_op.result(&ctx);
    let bitcast_op = BitcastOp::new(&mut ctx, w_res, i8);
    assert_eq!(bitcast_op.result(&ctx).get_type(&ctx), i8);

    let b_res = bitcast_op.result(&ctx);
    let out_op = OutputOp::new(&mut ctx, vec![b_res]);

    const_op.get_operation().insert_at_back(body, &mut ctx);
    wire_op.get_operation().insert_at_back(body, &mut ctx);
    bitcast_op.get_operation().insert_at_back(body, &mut ctx);
    out_op.get_operation().insert_at_back(body, &mut ctx);

    verify_op(&module, &ctx).expect("hw.module should verify");
}

#[test]
fn test_hw_concat_and_slice() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let (module, body) = create_test_module(&mut ctx, "concat_test");

    let i8: TypeHandle = IntegerType::get(&mut ctx, 8, Signedness::Signless).into();
    let i16: TypeHandle = IntegerType::get(&mut ctx, 16, Signedness::Signless).into();

    let a1 = int_attr(&mut ctx, 8, 0xAB);
    let c1 = ConstantOp::new(&mut ctx, a1);
    let a2 = int_attr(&mut ctx, 8, 0xCD);
    let c2 = ConstantOp::new(&mut ctx, a2);

    let r1 = c1.result(&ctx);
    let r2 = c2.result(&ctx);
    let concat_op = ConcatOp::new(&mut ctx, vec![r1, r2], i16);
    assert_eq!(concat_op.result(&ctx).get_type(&ctx), i16);

    let low_bit = int_attr(&mut ctx, 32, 4);
    let cc_res = concat_op.result(&ctx);
    let slice_op = SliceOp::new(&mut ctx, cc_res, low_bit, i8);
    assert_eq!(slice_op.result(&ctx).get_type(&ctx), i8);

    let s_res = slice_op.result(&ctx);
    let out_op = OutputOp::new(&mut ctx, vec![s_res]);

    c1.get_operation().insert_at_back(body, &mut ctx);
    c2.get_operation().insert_at_back(body, &mut ctx);
    concat_op.get_operation().insert_at_back(body, &mut ctx);
    slice_op.get_operation().insert_at_back(body, &mut ctx);
    out_op.get_operation().insert_at_back(body, &mut ctx);

    verify_op(&module, &ctx).expect("hw.module should verify");
}

#[test]
fn test_hw_array_operations() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let (module, body) = create_test_module(&mut ctx, "array_test");

    let i8: TypeHandle = IntegerType::get(&mut ctx, 8, Signedness::Signless).into();
    let arr_ty: TypeHandle = ArrayType::get(&mut ctx, 4, i8).into();

    let a0 = int_attr(&mut ctx, 8, 10);
    let e0 = ConstantOp::new(&mut ctx, a0);
    let a1 = int_attr(&mut ctx, 8, 20);
    let e1 = ConstantOp::new(&mut ctx, a1);
    let a2 = int_attr(&mut ctx, 8, 30);
    let e2 = ConstantOp::new(&mut ctx, a2);
    let a3 = int_attr(&mut ctx, 8, 40);
    let e3 = ConstantOp::new(&mut ctx, a3);

    let r0 = e0.result(&ctx);
    let r1 = e1.result(&ctx);
    let r2 = e2.result(&ctx);
    let r3 = e3.result(&ctx);
    let arr_create = ArrayCreateOp::new(
        &mut ctx,
        vec![r0, r1, r2, r3],
        arr_ty,
    );
    assert_eq!(arr_create.result(&ctx).get_type(&ctx), arr_ty);

    let a_idx = int_attr(&mut ctx, 2, 2);
    let idx = ConstantOp::new(&mut ctx, a_idx);
    let arr_val = arr_create.result(&ctx);
    let idx_val = idx.result(&ctx);
    let arr_get = ArrayGetOp::new(&mut ctx, arr_val, idx_val, i8);
    assert_eq!(arr_get.result(&ctx).get_type(&ctx), i8);

    let g_res = arr_get.result(&ctx);

    // Test ArrayInjectOp
    let a_new_elem = int_attr(&mut ctx, 8, 99);
    let new_elem = ConstantOp::new(&mut ctx, a_new_elem);
    let new_elem_res = new_elem.result(&ctx);
    let arr_inject = ArrayInjectOp::new(&mut ctx, arr_val, idx_val, new_elem_res, arr_ty);
    assert_eq!(arr_inject.result(&ctx).get_type(&ctx), arr_ty);

    let out_op = OutputOp::new(&mut ctx, vec![g_res]);

    e0.get_operation().insert_at_back(body, &mut ctx);
    e1.get_operation().insert_at_back(body, &mut ctx);
    e2.get_operation().insert_at_back(body, &mut ctx);
    e3.get_operation().insert_at_back(body, &mut ctx);
    arr_create.get_operation().insert_at_back(body, &mut ctx);
    idx.get_operation().insert_at_back(body, &mut ctx);
    arr_get.get_operation().insert_at_back(body, &mut ctx);
    new_elem.get_operation().insert_at_back(body, &mut ctx);
    arr_inject.get_operation().insert_at_back(body, &mut ctx);
    out_op.get_operation().insert_at_back(body, &mut ctx);

    verify_op(&module, &ctx).expect("hw.module should verify");
}

#[test]
fn test_hw_struct_operations() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let (module, body) = create_test_module(&mut ctx, "struct_test");

    let i8: TypeHandle = IntegerType::get(&mut ctx, 8, Signedness::Signless).into();
    let i16: TypeHandle = IntegerType::get(&mut ctx, 16, Signedness::Signless).into();

    let f_a: Identifier = "a".try_into().unwrap();
    let f_b: Identifier = "b".try_into().unwrap();

    let struct_ty: TypeHandle = StructType::get(&mut ctx, vec![
        StructField::new(f_a.clone(), i8),
        StructField::new(f_b.clone(), i16),
    ]).into();

    let a_val = int_attr(&mut ctx, 8, 1);
    let val_a = ConstantOp::new(&mut ctx, a_val);
    let b_val = int_attr(&mut ctx, 16, 2);
    let val_b = ConstantOp::new(&mut ctx, b_val);

    let res_a = val_a.result(&ctx);
    let res_b = val_b.result(&ctx);
    let struct_create = StructCreateOp::new(&mut ctx, vec![res_a, res_b], struct_ty);
    assert_eq!(struct_create.result(&ctx).get_type(&ctx), struct_ty);

    let sc_res1 = struct_create.result(&ctx);
    let extract_a = StructExtractOp::new(&mut ctx, sc_res1, "a".to_string().into(), i8);
    assert_eq!(extract_a.result(&ctx).get_type(&ctx), i8);

    let nva = int_attr(&mut ctx, 8, 99);
    let new_val_a = ConstantOp::new(&mut ctx, nva);
    let sc_res2 = struct_create.result(&ctx);
    let nva_res = new_val_a.result(&ctx);
    let inject_a = StructInjectOp::new(&mut ctx, sc_res2, "a".to_string().into(), nva_res, struct_ty);
    assert_eq!(inject_a.result(&ctx).get_type(&ctx), struct_ty);

    let sc_res3 = struct_create.result(&ctx);
    let explode = StructExplodeOp::new(&mut ctx, sc_res3, vec![i8, i16]);
    assert_eq!(explode.results(&ctx).len(), 2);

    let e_res = extract_a.result(&ctx);
    let out_op = OutputOp::new(&mut ctx, vec![e_res]);

    val_a.get_operation().insert_at_back(body, &mut ctx);
    val_b.get_operation().insert_at_back(body, &mut ctx);
    struct_create.get_operation().insert_at_back(body, &mut ctx);
    extract_a.get_operation().insert_at_back(body, &mut ctx);
    new_val_a.get_operation().insert_at_back(body, &mut ctx);
    inject_a.get_operation().insert_at_back(body, &mut ctx);
    explode.get_operation().insert_at_back(body, &mut ctx);
    out_op.get_operation().insert_at_back(body, &mut ctx);

    verify_op(&module, &ctx).expect("hw.module should verify");
}

#[test]
fn test_hw_extern_module_and_instance() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let ext_name: Identifier = "pll_clk_gen".try_into().unwrap();
    let extern_mod = ExternModuleOp::new(&mut ctx, ext_name.clone());
    verify_op(&extern_mod, &ctx).expect("hw.module_extern should verify");

    let i1: TypeHandle = IntegerType::get(&mut ctx, 1, Signedness::Signless).into();
    let a_clk = int_attr(&mut ctx, 1, 1);
    let clk_in = ConstantOp::new(&mut ctx, a_clk);

    let clk_res = clk_in.result(&ctx);
    let inst = InstanceOp::new(
        &mut ctx,
        "u_pll".to_string().into(),
        ext_name.into(),
        vec![clk_res],
        vec![i1],
    );
    assert_eq!(inst.results(&ctx).len(), 1);
}

#[test]
fn test_hw_native_types() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let hw_int: TypeHandle = IntType::get(&mut ctx, 64).into();
    let hw_inout: TypeHandle = InoutType::get(&mut ctx, hw_int).into();
    let alias_sym: Identifier = "my_custom_bus".try_into().unwrap();
    let hw_alias: TypeHandle = TypeAliasType::get(&mut ctx, alias_sym, hw_int).into();

    let ir_int = hw_int.disp(&ctx).to_string();
    assert_eq!(ir_int, "hw.int <64>");

    let ir_inout = hw_inout.disp(&ctx).to_string();
    assert_eq!(ir_inout, "hw.inout <hw.int <64>>");

    let ir_alias = hw_alias.disp(&ctx).to_string();
    assert!(ir_alias.contains("hw.typealias") && ir_alias.contains("my_custom_bus"));
}

#[test]
fn test_hw_enum_type() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let i2: TypeHandle = IntegerType::get(&mut ctx, 2, Signedness::Signless).into();
    let enum_name: Identifier = "Opcode".try_into().unwrap();
    let add_name: Identifier = "add".try_into().unwrap();
    let sub_name: Identifier = "sub".try_into().unwrap();
    let enum_ty = EnumType::get(
        &mut ctx,
        enum_name,
        i2,
        vec![EnumVariant::new(add_name.clone(), 0), EnumVariant::new(sub_name, 1)],
    );

    let enum_ref = enum_ty.deref(&ctx);
    assert_eq!(enum_ref.underlying_type(), i2);
    assert_eq!(enum_ref.get_variant(&add_name).unwrap().value, 0);
    assert_eq!(enum_ref.variants().len(), 2);
    assert!(enum_ref.disp(&ctx).to_string().contains("Opcode"));
}

#[test]
fn test_hw_union_operations() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let (module, body) = create_test_module(&mut ctx, "union_test");

    let i8: TypeHandle = IntegerType::get(&mut ctx, 8, Signedness::Signless).into();
    let i32: TypeHandle = IntegerType::get(&mut ctx, 32, Signedness::Signless).into();

    let f_byte: Identifier = "byte_val".try_into().unwrap();
    let f_word: Identifier = "word_val".try_into().unwrap();

    let union_ty: TypeHandle = UnionType::get(&mut ctx, vec![
        StructField::new(f_byte.clone(), i8),
        StructField::new(f_word.clone(), i32),
    ]).into();

    let b_attr = int_attr(&mut ctx, 8, 0x7F);
    let b_const = ConstantOp::new(&mut ctx, b_attr);
    let b_val = b_const.result(&ctx);

    let union_create = UnionCreateOp::new(&mut ctx, b_val, "byte_val".to_string().into(), union_ty);
    assert_eq!(union_create.result(&ctx).get_type(&ctx), union_ty);

    let u_res = union_create.result(&ctx);
    let union_extract = UnionExtractOp::new(&mut ctx, u_res, "byte_val".to_string().into(), i8);
    assert_eq!(union_extract.result(&ctx).get_type(&ctx), i8);

    let ext_res = union_extract.result(&ctx);
    let out_op = OutputOp::new(&mut ctx, vec![ext_res]);

    b_const.get_operation().insert_at_back(body, &mut ctx);
    union_create.get_operation().insert_at_back(body, &mut ctx);
    union_extract.get_operation().insert_at_back(body, &mut ctx);
    out_op.get_operation().insert_at_back(body, &mut ctx);

    verify_op(&module, &ctx).expect("hw.module with union ops should verify");
}

#[test]
fn test_hw_parameters_and_hierpath() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let param_name: Identifier = "DATA_WIDTH".try_into().unwrap();
    let param_decl = ParamDeclOp::new(
        &mut ctx,
        param_name,
        "i32".to_string().into(),
        "32".to_string().into(),
    );
    verify_op(&param_decl, &ctx).expect("hw.param_decl should verify");

    let i32_ty: TypeHandle = IntegerType::get(&mut ctx, 32, Signedness::Signless).into();
    let param_val = ParamValueOp::new(&mut ctx, "DATA_WIDTH".to_string().into(), i32_ty);
    assert_eq!(param_val.result(&ctx).get_type(&ctx), i32_ty);

    let path_name: Identifier = "npath_core_alu".try_into().unwrap();
    let hier_path = HierPathOp::new(
        &mut ctx,
        path_name,
        "top.cpu_tile.core.alu".to_string().into(),
    );
    verify_op(&hier_path, &ctx).expect("hw.hierpath should verify");
}

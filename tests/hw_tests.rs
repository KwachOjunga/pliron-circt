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
    op::{Op, verify_op},
    printable::Printable,
    r#type::TypeHandle,
    utils::apint::APInt,
};
use pliron_hw::{
    hw::{
        ops::{ConstantOp, ModuleOp, OutputOp},
        types::IntType,
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

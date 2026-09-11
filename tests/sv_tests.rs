// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

use pliron::{
    builtin::types::{IntegerType, Signedness},
    context::Context,
    op::{Op, verify_op},
    r#type::{TypeHandle, Typed},
};
use pliron_hw::{
    hw::ops::{ModuleOp, OutputOp},
    register_all,
    seq::types::{ClockType, ResetType},
    sv::ops::{AlwaysFfOp, AssignOp},
};

#[test]
// Proves that valid SV assignment and always_ff intent composes with seq types.
fn test_sv_emission_operations_verify() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let clock_ty: TypeHandle = ClockType::get(&mut ctx).into();
    let reset_ty: TypeHandle = ResetType::get(&mut ctx).into();
    let i8_ty: TypeHandle = IntegerType::get(&mut ctx, 8, Signedness::Signless).into();
    let module = ModuleOp::new(
        &mut ctx,
        "sv_valid".try_into().unwrap(),
        vec![clock_ty, reset_ty, i8_ty, i8_ty],
    );
    let body = module.get_body(&ctx);
    let clock = module.get_input(&ctx, 0);
    let reset = module.get_input(&ctx, 1);
    let input = module.get_input(&ctx, 2);
    let reset_value = module.get_input(&ctx, 3);

    let assign = AssignOp::new(&mut ctx, "next_value", input);
    let assigned_value = assign.result(&ctx);
    let always = AlwaysFfOp::new(
        &mut ctx,
        "state",
        clock,
        assigned_value,
        reset,
        reset_value,
        false,
        "active_high",
    );
    let output = OutputOp::new(&mut ctx, vec![assigned_value]);
    assign.get_operation().insert_at_back(body, &mut ctx);
    always.get_operation().insert_at_back(body, &mut ctx);
    output.get_operation().insert_at_back(body, &mut ctx);

    assert_eq!(assign.target(&ctx).as_ref(), "next_value");
    assert_eq!(assign.result(&ctx).get_type(&ctx), i8_ty);
    verify_op(&module, &ctx).expect("valid SV emission operations should verify");
}

#[test]
// Proves that SV emission rejects a non-clock event control before lowering.
fn test_sv_rejects_non_clock_always_ff() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let reset_ty: TypeHandle = ResetType::get(&mut ctx).into();
    let i8_ty: TypeHandle = IntegerType::get(&mut ctx, 8, Signedness::Signless).into();
    let module = ModuleOp::new(
        &mut ctx,
        "sv_invalid_clock".try_into().unwrap(),
        vec![i8_ty, reset_ty, i8_ty, i8_ty],
    );
    let body = module.get_body(&ctx);
    let invalid_clock = module.get_input(&ctx, 0);
    let reset = module.get_input(&ctx, 1);
    let input = module.get_input(&ctx, 2);
    let reset_value = module.get_input(&ctx, 3);
    let always = AlwaysFfOp::new(
        &mut ctx,
        "state",
        invalid_clock,
        input,
        reset,
        reset_value,
        false,
        "active_high",
    );
    always.get_operation().insert_at_back(body, &mut ctx);

    assert!(verify_op(&module, &ctx).is_err());
}

#[test]
// Proves that emission metadata rejects an unsupported reset polarity.
fn test_sv_rejects_unknown_reset_polarity() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let clock_ty: TypeHandle = ClockType::get(&mut ctx).into();
    let reset_ty: TypeHandle = ResetType::get(&mut ctx).into();
    let i8_ty: TypeHandle = IntegerType::get(&mut ctx, 8, Signedness::Signless).into();
    let module = ModuleOp::new(
        &mut ctx,
        "sv_invalid_polarity".try_into().unwrap(),
        vec![clock_ty, reset_ty, i8_ty, i8_ty],
    );
    let body = module.get_body(&ctx);
    let clock = module.get_input(&ctx, 0);
    let reset = module.get_input(&ctx, 1);
    let input = module.get_input(&ctx, 2);
    let reset_value = module.get_input(&ctx, 3);
    let always = AlwaysFfOp::new(
        &mut ctx,
        "state",
        clock,
        input,
        reset,
        reset_value,
        true,
        "active_middle",
    );
    always.get_operation().insert_at_back(body, &mut ctx);

    assert!(verify_op(&module, &ctx).is_err());
}

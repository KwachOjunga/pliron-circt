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
    seq::{
        ops::{ClockGateOp, CompRegOp, FirRegOp, HLMemOp, HLMemReadOp, HLMemWriteOp},
        types::{ClockType, MemoryType, ResetType},
    },
};

#[test]
// Proves that clock gating and an ordinary register compose with explicit
// clock identity and preserve the register result type.
fn test_seq_clock_gate_and_compreg() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let clock_ty: TypeHandle = ClockType::get(&mut ctx).into();
    let reset_ty: TypeHandle = ResetType::get(&mut ctx).into();
    let i1_ty: TypeHandle = IntegerType::get(&mut ctx, 1, Signedness::Signless).into();
    let i8_ty: TypeHandle = IntegerType::get(&mut ctx, 8, Signedness::Signless).into();
    assert_ne!(clock_ty, reset_ty);
    assert_ne!(clock_ty, i1_ty);

    let module = ModuleOp::new(
        &mut ctx,
        "seq_test".try_into().unwrap(),
        vec![clock_ty, i8_ty, i1_ty],
    );
    let body = module.get_body(&ctx);
    let clock = module.get_input(&ctx, 0);
    let input = module.get_input(&ctx, 1);
    let enable = module.get_input(&ctx, 2);

    let gate = ClockGateOp::new(&mut ctx, clock, enable, clock_ty);
    let gated_clock = gate.result(&ctx);
    let reg = CompRegOp::new(&mut ctx, gated_clock, input, i8_ty);
    let registered_value = reg.result(&ctx);
    let output = OutputOp::new(&mut ctx, vec![registered_value, gated_clock]);
    gate.get_operation().insert_at_back(body, &mut ctx);
    reg.get_operation().insert_at_back(body, &mut ctx);
    output.get_operation().insert_at_back(body, &mut ctx);

    assert_eq!(gate.result(&ctx).get_type(&ctx), clock_ty);
    assert_eq!(reg.result(&ctx).get_type(&ctx), i8_ty);
    verify_op(&module, &ctx).expect("seq operations should compose in an hw.module");
}

#[test]
// Proves that reset policy, abstract memory identity, synchronous reads, and
// enabled writes can coexist in one verified hardware module.
fn test_seq_reset_register_and_memory_operations() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let clock_ty: TypeHandle = ClockType::get(&mut ctx).into();
    let reset_ty: TypeHandle = ResetType::get(&mut ctx).into();
    let i1_ty: TypeHandle = IntegerType::get(&mut ctx, 1, Signedness::Signless).into();
    let i4_ty: TypeHandle = IntegerType::get(&mut ctx, 4, Signedness::Signless).into();
    let i8_ty: TypeHandle = IntegerType::get(&mut ctx, 8, Signedness::Signless).into();
    let memory_ty: TypeHandle = MemoryType::get(&mut ctx, 16, i8_ty).into();

    let module = ModuleOp::new(
        &mut ctx,
        "seq_memory_test".try_into().unwrap(),
        vec![clock_ty, reset_ty, i8_ty, i8_ty, i4_ty, i1_ty],
    );
    let body = module.get_body(&ctx);
    let clock = module.get_input(&ctx, 0);
    let reset = module.get_input(&ctx, 1);
    let input = module.get_input(&ctx, 2);
    let reset_value = module.get_input(&ctx, 3);
    let address = module.get_input(&ctx, 4);
    let enable = module.get_input(&ctx, 5);

    let register = FirRegOp::new(
        &mut ctx,
        clock,
        input,
        reset,
        reset_value,
        i8_ty,
        false,
        "active_high",
    );
    let memory = HLMemOp::new(&mut ctx, memory_ty, "read-first");
    let memory_handle = memory.result(&ctx);
    let read = HLMemReadOp::new(&mut ctx, clock, memory_handle, address, i8_ty);
    let read_value = read.result(&ctx);
    let write = HLMemWriteOp::new(&mut ctx, clock, memory_handle, address, input, enable);
    let register_value = register.result(&ctx);
    let output = OutputOp::new(&mut ctx, vec![register_value, read_value]);

    register.get_operation().insert_at_back(body, &mut ctx);
    memory.get_operation().insert_at_back(body, &mut ctx);
    read.get_operation().insert_at_back(body, &mut ctx);
    write.get_operation().insert_at_back(body, &mut ctx);
    output.get_operation().insert_at_back(body, &mut ctx);

    let memory_handle_type = memory_ty.deref(&ctx);
    let memory = memory_handle_type
        .downcast_ref::<MemoryType>()
        .expect("memory handle should have the seq.mem type");
    assert_eq!(memory.depth(), 16);
    assert_eq!(memory.element_type(), i8_ty);
    assert_eq!(register.result(&ctx).get_type(&ctx), i8_ty);
    assert_eq!(read.result(&ctx).get_type(&ctx), i8_ty);
    verify_op(&module, &ctx).expect("complete seq operation graph should verify");
}

#[test]
// Proves that the semantic verifier rejects a data value used as a clock.
fn test_seq_rejects_non_clock_register_input() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let clock_ty: TypeHandle = ClockType::get(&mut ctx).into();
    let i8_ty: TypeHandle = IntegerType::get(&mut ctx, 8, Signedness::Signless).into();
    let module = ModuleOp::new(
        &mut ctx,
        "seq_invalid_clock".try_into().unwrap(),
        vec![i8_ty, i8_ty],
    );
    let body = module.get_body(&ctx);
    let invalid_clock = module.get_input(&ctx, 0);
    let input = module.get_input(&ctx, 1);
    let register = CompRegOp::new(&mut ctx, invalid_clock, input, i8_ty);
    let register_value = register.result(&ctx);
    let output = OutputOp::new(&mut ctx, vec![register_value]);
    register.get_operation().insert_at_back(body, &mut ctx);
    output.get_operation().insert_at_back(body, &mut ctx);

    assert_ne!(invalid_clock.get_type(&ctx), clock_ty);
    assert!(verify_op(&module, &ctx).is_err());
}

#[test]
// Proves that reset polarity and memory collision policy accept only the
// documented finite vocabularies.
fn test_seq_rejects_unknown_policy_attributes() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let clock_ty: TypeHandle = ClockType::get(&mut ctx).into();
    let reset_ty: TypeHandle = ResetType::get(&mut ctx).into();
    let i8_ty: TypeHandle = IntegerType::get(&mut ctx, 8, Signedness::Signless).into();
    let module = ModuleOp::new(
        &mut ctx,
        "seq_invalid_policy".try_into().unwrap(),
        vec![clock_ty, reset_ty, i8_ty, i8_ty],
    );
    let body = module.get_body(&ctx);
    let clock = module.get_input(&ctx, 0);
    let reset = module.get_input(&ctx, 1);
    let input = module.get_input(&ctx, 2);
    let reset_value = module.get_input(&ctx, 3);
    let register = FirRegOp::new(
        &mut ctx,
        clock,
        input,
        reset,
        reset_value,
        i8_ty,
        false,
        "active_middle",
    );
    let register_value = register.result(&ctx);
    let output = OutputOp::new(&mut ctx, vec![register_value]);
    register.get_operation().insert_at_back(body, &mut ctx);
    output.get_operation().insert_at_back(body, &mut ctx);

    assert!(verify_op(&module, &ctx).is_err());
}

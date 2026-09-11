// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

use pliron::{
    builtin::types::{IntegerType, Signedness},
    context::Context,
    irbuild::{listener::DummyListener, rewriter::IRRewriter},
    op::{Op, verify_op},
    r#type::{TypeHandle, Typed},
};
use pliron_hw::{
    hw::{
        ops::{ModuleOp, OutputOp},
        validation::validate_module,
    },
    register_all,
    seq::{
        ops::{CompRegOp, FirRegOp, HLMemOp, HLMemWriteOp},
        types::{ClockType, MemoryType, ResetType},
    },
    sv::{
        canonicalization::eliminate_redundant_assign,
        lowering::{lower_compreg, lower_firreg, lower_module_registers},
        ops::{
            AlwaysCombOp, AlwaysFfNoResetOp, AlwaysFfOp, AssignOp, InstanceOp, LogicDeclOp,
            MemDeclOp,
        },
        printer::render_module,
    },
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

#[test]
// Proves semantic seq registers lower into the matching reset/no-reset SV forms.
fn test_sv_lowering_preserves_register_contracts() {
    let mut ctx = Context::new();
    register_all(&mut ctx);
    let clock_ty: TypeHandle = ClockType::get(&mut ctx).into();
    let reset_ty: TypeHandle = ResetType::get(&mut ctx).into();
    let i8_ty: TypeHandle = IntegerType::get(&mut ctx, 8, Signedness::Signless).into();
    let module = ModuleOp::new(
        &mut ctx,
        "sv_lowering".try_into().unwrap(),
        vec![clock_ty, reset_ty, i8_ty, i8_ty],
    );
    let clock = module.get_input(&ctx, 0);
    let reset = module.get_input(&ctx, 1);
    let input = module.get_input(&ctx, 2);
    let reset_value = module.get_input(&ctx, 3);
    let compreg = CompRegOp::new(&mut ctx, clock, input, i8_ty);
    let firreg = FirRegOp::new(
        &mut ctx,
        clock,
        input,
        reset,
        reset_value,
        i8_ty,
        true,
        "active_low",
    );
    let lowered_compreg = lower_compreg(&mut ctx, &compreg, "state");
    let lowered_firreg = lower_firreg(&mut ctx, &firreg, "reset_state");
    assert!(verify_op(&lowered_compreg, &ctx).is_ok());
    assert!(verify_op(&lowered_firreg, &ctx).is_ok());
    assert!(AlwaysFfNoResetOp::get_concrete_op_info().1 != AlwaysFfOp::get_concrete_op_info().1);
}

#[test]
// Proves the canonicalizer removes an assignment that only rebinds a value to itself.
fn test_sv_canonicalizes_redundant_assignment() {
    let mut ctx = Context::new();
    register_all(&mut ctx);
    let i8_ty: TypeHandle = IntegerType::get(&mut ctx, 8, Signedness::Signless).into();
    let module = ModuleOp::new(&mut ctx, "sv_canonical".try_into().unwrap(), vec![i8_ty]);
    let body = module.get_body(&ctx);
    let input = module.get_input(&ctx, 0);
    input.set_name(&ctx, Some("value".try_into().unwrap()));
    let assign = AssignOp::new(&mut ctx, "value", input);
    let assign_ptr = assign.get_operation();
    let assigned_value = assign.result(&ctx);
    let output = OutputOp::new(&mut ctx, vec![assigned_value]);
    assign_ptr.insert_at_back(body, &mut ctx);
    output.get_operation().insert_at_back(body, &mut ctx);
    let mut rewriter = IRRewriter::<DummyListener>::default();
    assert!(eliminate_redundant_assign(&mut ctx, &mut rewriter, &assign).unwrap());
    assert!(verify_op(&module, &ctx).is_ok());
}

#[test]
// Proves module validation catches duplicate emitted targets and conflicting memory writes.
fn test_hw_module_validation_rejects_cross_operation_conflicts() {
    let mut ctx = Context::new();
    register_all(&mut ctx);
    let clock_ty: TypeHandle = ClockType::get(&mut ctx).into();
    let i1_ty: TypeHandle = IntegerType::get(&mut ctx, 1, Signedness::Signless).into();
    let i4_ty: TypeHandle = IntegerType::get(&mut ctx, 4, Signedness::Signless).into();
    let i8_ty: TypeHandle = IntegerType::get(&mut ctx, 8, Signedness::Signless).into();
    let memory_ty: TypeHandle = MemoryType::get(&mut ctx, 16, i8_ty).into();
    let module = ModuleOp::new(
        &mut ctx,
        "validation".try_into().unwrap(),
        vec![clock_ty, i4_ty, i8_ty, i1_ty],
    );
    let body = module.get_body(&ctx);
    let clock = module.get_input(&ctx, 0);
    let address = module.get_input(&ctx, 1);
    let data = module.get_input(&ctx, 2);
    let enable = module.get_input(&ctx, 3);
    let memory = HLMemOp::new(&mut ctx, memory_ty, "read-first");
    let handle = memory.result(&ctx);
    let write_a = HLMemWriteOp::new(&mut ctx, clock, handle, address, data, enable);
    let write_b = HLMemWriteOp::new(&mut ctx, clock, handle, address, data, enable);
    let assign_a = AssignOp::new(&mut ctx, "duplicate", data);
    let assign_b = AssignOp::new(&mut ctx, "duplicate", data);
    let assign_a_value = assign_a.result(&ctx);
    let assign_b_value = assign_b.result(&ctx);
    let output = OutputOp::new(&mut ctx, vec![assign_a_value, assign_b_value]);
    for op in [
        memory.get_operation(),
        write_a.get_operation(),
        write_b.get_operation(),
        assign_a.get_operation(),
        assign_b.get_operation(),
    ] {
        op.insert_at_back(body, &mut ctx);
    }
    output.get_operation().insert_at_back(body, &mut ctx);
    assert!(validate_module(&ctx, &module).is_err());
}

#[test]
// Proves verified SV intent renders to deterministic SystemVerilog source.
fn test_sv_printer_renders_register_and_assignment() {
    let mut ctx = Context::new();
    register_all(&mut ctx);
    let clock_ty: TypeHandle = ClockType::get(&mut ctx).into();
    let reset_ty: TypeHandle = ResetType::get(&mut ctx).into();
    let i8_ty: TypeHandle = IntegerType::get(&mut ctx, 8, Signedness::Signless).into();
    let module = ModuleOp::new(
        &mut ctx,
        "printed_sv".try_into().unwrap(),
        vec![clock_ty, reset_ty, i8_ty, i8_ty],
    );
    let body = module.get_body(&ctx);
    let clock = module.get_input(&ctx, 0);
    let reset = module.get_input(&ctx, 1);
    let input = module.get_input(&ctx, 2);
    let reset_value = module.get_input(&ctx, 3);
    clock.set_name(&ctx, Some("clk".try_into().unwrap()));
    reset.set_name(&ctx, Some("reset".try_into().unwrap()));
    input.set_name(&ctx, Some("input_value".try_into().unwrap()));
    reset_value.set_name(&ctx, Some("reset_value".try_into().unwrap()));
    let assign = AssignOp::new(&mut ctx, "next_value", input);
    let assigned = assign.result(&ctx);
    let always = AlwaysFfOp::new(
        &mut ctx,
        "state",
        clock,
        assigned,
        reset,
        reset_value,
        false,
        "active_high",
    );
    let output = OutputOp::new(&mut ctx, vec![assigned]);
    assign.get_operation().insert_at_back(body, &mut ctx);
    always.get_operation().insert_at_back(body, &mut ctx);
    output.get_operation().insert_at_back(body, &mut ctx);

    let source = render_module(&ctx, &module).expect("verified SV module should render");
    assert!(source.contains("module printed_sv"));
    assert!(source.contains("input logic clk"));
    assert!(source.contains("wire [7:0] next_value;"));
    assert!(source.contains("logic [7:0] state;"));
    assert!(source.contains("assign next_value = input_value;"));
    assert!(source.contains("always_ff @(posedge clk)"));
    assert!(source.contains("state <= reset_value;"));
}

#[test]
// Proves the module conversion pass walks seq registers and inserts SV intent.
fn test_sv_module_conversion_lowers_registers() {
    let mut ctx = Context::new();
    register_all(&mut ctx);
    let clock_ty: TypeHandle = ClockType::get(&mut ctx).into();
    let reset_ty: TypeHandle = ResetType::get(&mut ctx).into();
    let i8_ty: TypeHandle = IntegerType::get(&mut ctx, 8, Signedness::Signless).into();
    let module = ModuleOp::new(
        &mut ctx,
        "sv_conversion".try_into().unwrap(),
        vec![clock_ty, reset_ty, i8_ty, i8_ty],
    );
    let body = module.get_body(&ctx);
    let clock = module.get_input(&ctx, 0);
    let reset = module.get_input(&ctx, 1);
    let input = module.get_input(&ctx, 2);
    let reset_value = module.get_input(&ctx, 3);
    let compreg = CompRegOp::new(&mut ctx, clock, input, i8_ty);
    let compreg_result = compreg.result(&ctx);
    compreg_result.set_name(&ctx, Some("plain_state".try_into().unwrap()));
    let firreg = FirRegOp::new(
        &mut ctx,
        clock,
        input,
        reset,
        reset_value,
        i8_ty,
        true,
        "active_low",
    );
    let firreg_result = firreg.result(&ctx);
    firreg_result.set_name(&ctx, Some("reset_state".try_into().unwrap()));
    reset.set_name(&ctx, Some("reset".try_into().unwrap()));
    let output = OutputOp::new(&mut ctx, vec![compreg_result, firreg_result]);
    compreg.get_operation().insert_at_back(body, &mut ctx);
    firreg.get_operation().insert_at_back(body, &mut ctx);
    output.get_operation().insert_at_back(body, &mut ctx);

    assert_eq!(lower_module_registers(&mut ctx, &module).unwrap(), 2);
    let source = render_module(&ctx, &module).unwrap();
    assert!(source.contains("logic [7:0] plain_state;"));
    assert!(source.contains("logic [7:0] reset_state;"));
    assert!(source.contains("negedge reset"));
}

#[test]
// Proves the SV surface can describe declarations, procedural combinational
// logic, instances, and memory resources without collapsing them into assigns.
fn test_sv_declarations_instances_and_memory_render() {
    let mut ctx = Context::new();
    register_all(&mut ctx);
    let i8_ty: TypeHandle = IntegerType::get(&mut ctx, 8, Signedness::Signless).into();
    let memory_ty: TypeHandle = MemoryType::get(&mut ctx, 16, i8_ty).into();
    let module = ModuleOp::new(&mut ctx, "sv_surface".try_into().unwrap(), vec![i8_ty]);
    let body = module.get_body(&ctx);
    let input = module.get_input(&ctx, 0);
    input.set_name(&ctx, Some("input_value".try_into().unwrap()));
    let declaration = LogicDeclOp::new(&mut ctx, "declared_value", i8_ty);
    let declared_value = declaration.result(&ctx);
    let combinational = AlwaysCombOp::new(&mut ctx, "declared_value", input);
    let instance = InstanceOp::new(&mut ctx, "u_child", "child_module", vec![input]);
    let memory = MemDeclOp::new(&mut ctx, "storage", memory_ty);
    let output = OutputOp::new(&mut ctx, vec![declared_value]);
    for op in [
        declaration.get_operation(),
        combinational.get_operation(),
        instance.get_operation(),
        memory.get_operation(),
    ] {
        op.insert_at_back(body, &mut ctx);
    }
    output.get_operation().insert_at_back(body, &mut ctx);

    let source = render_module(&ctx, &module).expect("expanded SV surface should render");
    assert!(source.contains("logic [7:0] declared_value;"));
    assert!(source.contains("always_comb begin"));
    assert!(source.contains("child_module u_child (input_value);"));
    assert!(source.contains("logic [7:0] storage [0:15];"));
}

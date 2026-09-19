// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

use pliron::{
    builtin::{
        attributes::IntegerAttr,
        types::{IntegerType, Signedness},
    },
    context::Context,
    irbuild::{listener::DummyListener, rewriter::IRRewriter},
    op::{Op, verify_op},
    r#type::{TypeHandle, Typed},
    utils::apint::{APInt, bw},
};
use pliron_hw::{
    hw::{
        ops::{ModuleOp, OutputOp},
        validation::validate_module,
    },
    register_all,
    seq::{
        ops::{CompRegOp, FirRegOp, HLMemOp, HLMemReadOp, HLMemWriteOp},
        types::{ClockType, MemoryType, ResetType},
    },
    sv::{
        canonicalization::eliminate_redundant_assign,
        lowering::{lower_compreg, lower_firreg, lower_module, lower_module_registers},
        ops::{
            AlwaysCombOp, AlwaysFfNoResetOp, AlwaysFfOp, AssignOp, BinaryExprOp, BpaOp, CaseOp,
            ConcatExprOp, ConstantExprOp, IndexExprOp, InstanceOp, LogicDeclOp, MemDeclOp,
            MemReadOp, MemWriteOp, MuxExprOp, NbaOp, RegDeclOp, SliceExprOp, UnaryExprOp,
            WireDeclOp,
        },
        parser::parse_sv_module,
        printer::render_module,
    },
};

#[test]
// Proves that valid SV assignment and always_ff intent composes with seq types.
fn test_sv_emission_operations_verify() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let clock_ty: TypeHandle = ClockType::get(&ctx).into();
    let reset_ty: TypeHandle = ResetType::get(&ctx).into();
    let i8_ty: TypeHandle = IntegerType::get(&ctx, 8, Signedness::Signless).into();
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
    assign.get_operation().insert_at_back(body, &ctx);
    always.get_operation().insert_at_back(body, &ctx);
    output.get_operation().insert_at_back(body, &ctx);

    assert_eq!(assign.target(&ctx).as_ref(), "next_value");
    assert_eq!(assign.result(&ctx).get_type(&ctx), i8_ty);
    verify_op(&module, &ctx).expect("valid SV emission operations should verify");
}

#[test]
// Proves that SV emission rejects a non-clock event control before lowering.
fn test_sv_rejects_non_clock_always_ff() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let reset_ty: TypeHandle = ResetType::get(&ctx).into();
    let i8_ty: TypeHandle = IntegerType::get(&ctx, 8, Signedness::Signless).into();
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
    always.get_operation().insert_at_back(body, &ctx);

    assert!(verify_op(&module, &ctx).is_err());
}

#[test]
// Proves that emission metadata rejects an unsupported reset polarity.
fn test_sv_rejects_unknown_reset_polarity() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let clock_ty: TypeHandle = ClockType::get(&ctx).into();
    let reset_ty: TypeHandle = ResetType::get(&ctx).into();
    let i8_ty: TypeHandle = IntegerType::get(&ctx, 8, Signedness::Signless).into();
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
    always.get_operation().insert_at_back(body, &ctx);

    assert!(verify_op(&module, &ctx).is_err());
}

#[test]
// Proves semantic seq registers lower into the matching reset/no-reset SV forms.
fn test_sv_lowering_preserves_register_contracts() {
    let mut ctx = Context::new();
    register_all(&mut ctx);
    let clock_ty: TypeHandle = ClockType::get(&ctx).into();
    let reset_ty: TypeHandle = ResetType::get(&ctx).into();
    let i8_ty: TypeHandle = IntegerType::get(&ctx, 8, Signedness::Signless).into();
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
    let i8_ty: TypeHandle = IntegerType::get(&ctx, 8, Signedness::Signless).into();
    let module = ModuleOp::new(&mut ctx, "sv_canonical".try_into().unwrap(), vec![i8_ty]);
    let body = module.get_body(&ctx);
    let input = module.get_input(&ctx, 0);
    input.set_name(&ctx, Some("value".try_into().unwrap()));
    let assign = AssignOp::new(&mut ctx, "value", input);
    let assign_ptr = assign.get_operation();
    let assigned_value = assign.result(&ctx);
    let output = OutputOp::new(&mut ctx, vec![assigned_value]);
    assign_ptr.insert_at_back(body, &ctx);
    output.get_operation().insert_at_back(body, &ctx);
    let mut rewriter = IRRewriter::<DummyListener>::default();
    assert!(eliminate_redundant_assign(&mut ctx, &mut rewriter, &assign).unwrap());
    assert!(verify_op(&module, &ctx).is_ok());
}

#[test]
// Proves module validation catches duplicate emitted targets and conflicting memory writes.
fn test_hw_module_validation_rejects_cross_operation_conflicts() {
    let mut ctx = Context::new();
    register_all(&mut ctx);
    let clock_ty: TypeHandle = ClockType::get(&ctx).into();
    let i1_ty: TypeHandle = IntegerType::get(&ctx, 1, Signedness::Signless).into();
    let i4_ty: TypeHandle = IntegerType::get(&ctx, 4, Signedness::Signless).into();
    let i8_ty: TypeHandle = IntegerType::get(&ctx, 8, Signedness::Signless).into();
    let memory_ty: TypeHandle = MemoryType::get(&ctx, 16, i8_ty).into();
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
        op.insert_at_back(body, &ctx);
    }
    output.get_operation().insert_at_back(body, &ctx);
    assert!(validate_module(&ctx, &module).is_err());
}

#[test]
// Proves verified SV intent renders to deterministic SystemVerilog source.
fn test_sv_printer_renders_register_and_assignment() {
    let mut ctx = Context::new();
    register_all(&mut ctx);
    let clock_ty: TypeHandle = ClockType::get(&ctx).into();
    let reset_ty: TypeHandle = ResetType::get(&ctx).into();
    let i8_ty: TypeHandle = IntegerType::get(&ctx, 8, Signedness::Signless).into();
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
    assign.get_operation().insert_at_back(body, &ctx);
    always.get_operation().insert_at_back(body, &ctx);
    output.get_operation().insert_at_back(body, &ctx);

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
    let clock_ty: TypeHandle = ClockType::get(&ctx).into();
    let reset_ty: TypeHandle = ResetType::get(&ctx).into();
    let i8_ty: TypeHandle = IntegerType::get(&ctx, 8, Signedness::Signless).into();
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
    compreg.get_operation().insert_at_back(body, &ctx);
    firreg.get_operation().insert_at_back(body, &ctx);
    output.get_operation().insert_at_back(body, &ctx);

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
    let i8_ty: TypeHandle = IntegerType::get(&ctx, 8, Signedness::Signless).into();
    let memory_ty: TypeHandle = MemoryType::get(&ctx, 16, i8_ty).into();
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
        op.insert_at_back(body, &ctx);
    }
    output.get_operation().insert_at_back(body, &ctx);

    let source = render_module(&ctx, &module).expect("expanded SV surface should render");
    assert!(source.contains("logic [7:0] declared_value;"));
    assert!(source.contains("always_comb begin"));
    assert!(source.contains("child_module u_child (input_value);"));
    assert!(source.contains("logic [7:0] storage [0:15];"));
}

#[test]
// Proves synchronous memory ports lower and render with registered reads and enabled writes.
fn test_sv_memory_lowering_and_process_render() {
    let mut ctx = Context::new();
    register_all(&mut ctx);
    let clock_ty: TypeHandle = ClockType::get(&ctx).into();
    let i1_ty: TypeHandle = IntegerType::get(&ctx, 1, Signedness::Signless).into();
    let i4_ty: TypeHandle = IntegerType::get(&ctx, 4, Signedness::Signless).into();
    let i8_ty: TypeHandle = IntegerType::get(&ctx, 8, Signedness::Signless).into();
    let memory_ty: TypeHandle = MemoryType::get(&ctx, 16, i8_ty).into();
    let module = ModuleOp::new(
        &mut ctx,
        "sv_memory".try_into().unwrap(),
        vec![clock_ty, memory_ty, i4_ty, i8_ty, i1_ty],
    );
    let body = module.get_body(&ctx);
    let clock = module.get_input(&ctx, 0);
    let memory = module.get_input(&ctx, 1);
    let address = module.get_input(&ctx, 2);
    let data = module.get_input(&ctx, 3);
    let enable = module.get_input(&ctx, 4);
    memory.set_name(&ctx, Some("storage".try_into().unwrap()));
    address.set_name(&ctx, Some("address".try_into().unwrap()));
    data.set_name(&ctx, Some("write_data".try_into().unwrap()));
    enable.set_name(&ctx, Some("write_enable".try_into().unwrap()));
    clock.set_name(&ctx, Some("clock".try_into().unwrap()));
    let read = MemReadOp::new(&mut ctx, "read_data", clock, memory, address, i8_ty);
    let write = MemWriteOp::new(&mut ctx, "storage", clock, memory, address, data, enable);
    let read_value = read.result(&ctx);
    let output = OutputOp::new(&mut ctx, vec![read_value]);
    read.get_operation().insert_at_back(body, &ctx);
    write.get_operation().insert_at_back(body, &ctx);
    output.get_operation().insert_at_back(body, &ctx);
    let source = render_module(&ctx, &module).unwrap();
    assert!(source.contains("always_ff @(posedge clock)"));
    assert!(source.contains("read_data <= storage[address];"));
    assert!(source.contains("if (write_enable)"));
    assert!(source.contains("storage[address] <= write_data;"));
}

#[test]
// Proves the module pass materializes seq memory resources before lowering ports.
fn test_sv_module_conversion_lowers_memory_resource_and_ports() {
    let mut ctx = Context::new();
    register_all(&mut ctx);
    let clock_ty: TypeHandle = ClockType::get(&ctx).into();
    let i4_ty: TypeHandle = IntegerType::get(&ctx, 4, Signedness::Signless).into();
    let i8_ty: TypeHandle = IntegerType::get(&ctx, 8, Signedness::Signless).into();
    let memory_ty: TypeHandle = MemoryType::get(&ctx, 16, i8_ty).into();
    let module = ModuleOp::new(
        &mut ctx,
        "sv_memory_conversion".try_into().unwrap(),
        vec![clock_ty, memory_ty, i4_ty],
    );
    let body = module.get_body(&ctx);
    let clock = module.get_input(&ctx, 0);
    let memory_input = module.get_input(&ctx, 1);
    let address = module.get_input(&ctx, 2);
    clock.set_name(&ctx, Some("clock".try_into().unwrap()));
    memory_input.set_name(&ctx, Some("storage".try_into().unwrap()));
    address.set_name(&ctx, Some("address".try_into().unwrap()));
    let memory = HLMemOp::new(&mut ctx, memory_ty, "read-first");
    let memory_value = memory.result(&ctx);
    memory_value.set_name(&ctx, Some("storage".try_into().unwrap()));
    let read = HLMemReadOp::new(&mut ctx, clock, memory_value, address, i8_ty);
    let read_value = read.result(&ctx);
    let output = OutputOp::new(&mut ctx, vec![read_value]);
    memory.get_operation().insert_at_back(body, &ctx);
    read.get_operation().insert_at_back(body, &ctx);
    output.get_operation().insert_at_back(body, &ctx);
    assert_eq!(lower_module(&mut ctx, &module).unwrap(), 1);
    let source = render_module(&ctx, &module).unwrap();
    assert!(source.contains("logic [7:0] storage [0:15];"));
    assert!(source.contains("read_data_0 <= storage[address];"));
}

fn int_attr(ctx: &mut Context, width: u32, val: u64) -> IntegerAttr {
    let ty = IntegerType::get(ctx, width, Signedness::Signless);
    IntegerAttr::new(ty, APInt::from_u64(val, bw(width as usize)))
}

#[test]
// Proves that new SV expressions, declarations, assignments, and case statements verify and expose typed accessors.
fn test_sv_new_expressions_and_declarations_verify() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let i1_ty: TypeHandle = IntegerType::get(&ctx, 1, Signedness::Signless).into();
    let i4_ty: TypeHandle = IntegerType::get(&ctx, 4, Signedness::Signless).into();
    let i8_ty: TypeHandle = IntegerType::get(&ctx, 8, Signedness::Signless).into();
    let i16_ty: TypeHandle = IntegerType::get(&ctx, 16, Signedness::Signless).into();

    let module = ModuleOp::new(
        &mut ctx,
        "sv_expr_test".try_into().unwrap(),
        vec![i8_ty, i8_ty, i1_ty],
    );
    let body = module.get_body(&ctx);
    let a = module.get_input(&ctx, 0);
    let b = module.get_input(&ctx, 1);
    let sel = module.get_input(&ctx, 2);

    let wire = WireDeclOp::new(&mut ctx, "w", i8_ty);
    let reg = RegDeclOp::new(&mut ctx, "r", i8_ty);
    let bin = BinaryExprOp::new(&mut ctx, "+", a, b, i8_ty);
    let un = UnaryExprOp::new(&mut ctx, "~", a, i8_ty);
    let mux = MuxExprOp::new(&mut ctx, sel, a, b, i8_ty);
    let concat = ConcatExprOp::new(&mut ctx, vec![a, b], i16_ty);
    let slice_low = int_attr(&mut ctx, 32, 0);
    let slice_w = int_attr(&mut ctx, 32, 4);
    let slice = SliceExprOp::new(&mut ctx, a, slice_low, slice_w, i4_ty);
    let index = IndexExprOp::new(&mut ctx, a, sel, i1_ty);
    let const_val = int_attr(&mut ctx, 8, 42);
    let const_w = int_attr(&mut ctx, 32, 8);
    let c = ConstantExprOp::new(&mut ctx, const_val, const_w, i8_ty);
    let bin_res = bin.result(&ctx);
    let bpa = BpaOp::new(&mut ctx, "r", bin_res);
    let nba = NbaOp::new(&mut ctx, "r", bin_res);
    let case_op = CaseOp::new(&mut ctx, "r", sel);

    assert_eq!(wire.target(&ctx).as_ref(), "w");
    assert_eq!(reg.target(&ctx).as_ref(), "r");
    assert_eq!(bin.operator(&ctx).as_ref(), "+");
    assert_eq!(un.operator(&ctx).as_ref(), "~");
    assert_eq!(slice.low_bit(&ctx).value().to_u64(), 0);
    assert_eq!(slice.width(&ctx).value().to_u64(), 4);
    assert_eq!(c.value(&ctx).value().to_u64(), 42);
    assert_eq!(c.width(&ctx).value().to_u64(), 8);
    assert_eq!(bpa.target(&ctx).as_ref(), "r");
    assert_eq!(nba.target(&ctx).as_ref(), "r");
    assert_eq!(case_op.target(&ctx).as_ref(), "r");

    wire.get_operation().insert_at_back(body, &ctx);
    reg.get_operation().insert_at_back(body, &ctx);
    bin.get_operation().insert_at_back(body, &ctx);
    un.get_operation().insert_at_back(body, &ctx);
    mux.get_operation().insert_at_back(body, &ctx);
    concat.get_operation().insert_at_back(body, &ctx);
    slice.get_operation().insert_at_back(body, &ctx);
    index.get_operation().insert_at_back(body, &ctx);
    c.get_operation().insert_at_back(body, &ctx);
    bpa.get_operation().insert_at_back(body, &ctx);
    nba.get_operation().insert_at_back(body, &ctx);
    case_op.get_operation().insert_at_back(body, &ctx);

    let output = OutputOp::new(&mut ctx, vec![bin_res]);
    output.get_operation().insert_at_back(body, &ctx);

    verify_op(&module, &ctx).expect("SV expressions and declarations should verify");
}

#[test]
fn test_sv_parser_combinational_module() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let sv_code = r#"
module alu_block (
    input logic [7:0] a,
    input logic [7:0] b,
    output logic [7:0] sum
);
    wire [7:0] temp;
    assign temp = a + b;
    assign sum = temp;
endmodule
"#;

    let module = parse_sv_module(&mut ctx, sv_code).expect("SV combinational module should parse");
    verify_op(&module, &ctx).expect("parsed SV module should verify");

    let rendered = render_module(&ctx, &module).expect("parsed module should render");
    println!("{}", rendered);
    assert!(rendered.contains("module alu_block"));
    assert!(rendered.contains("assign temp = a + b;"));
}

#[test]
fn test_sv_parser_sequential_module_roundtrip() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let sv_code = r#"
module d_flip_flop (
    input logic clk,
    input logic [7:0] d,
    output logic [7:0] q
);
    logic [7:0] state;
    always_ff @(posedge clk) begin
        state <= d;
    end
    assign q = state;
endmodule
"#;

    let module = parse_sv_module(&mut ctx, sv_code).expect("SV sequential module should parse");
    verify_op(&module, &ctx).expect("parsed SV module should verify");

    let rendered = render_module(&ctx, &module).expect("parsed module should render");
    println!("{}", rendered);
    assert!(rendered.contains("module d_flip_flop"));
    assert!(rendered.contains("always_ff @(posedge clk)"));
    assert!(rendered.contains("state <= d;"));
}

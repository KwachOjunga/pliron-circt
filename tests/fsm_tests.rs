// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

use awint::bw;
use pliron::{
    builtin::{
        attributes::IntegerAttr,
        types::{IntegerType, Signedness},
    },
    context::Context,
    op::{Op, verify_op},
    r#type::TypeHandle,
    utils::apint::APInt,
};
use pliron_circt::{
    fsm::{
        canonicalization::{
            compute_reachable_states, eliminate_unreachable_states, find_unreachable_states,
        },
        lowering::{StateEncoding, plan_fsm_lowering},
        ops::{
            HWInstanceOp, InstanceOp, MachineOp, OutputOp, ReturnOp, StateOp, TransitionOp,
            TriggerOp, UpdateOp, VariableOp,
        },
        types::{InstanceType, InstanceTypeType, is_instance_type},
    },
    hw::ops::ModuleOp,
    register_all,
    seq::types::ClockType,
};

fn int_attr(ctx: &mut Context, width: u32, val: u64) -> IntegerAttr {
    let ty = IntegerType::get(ctx, width, Signedness::Signless);
    IntegerAttr::new(ty, APInt::from_u64(val, bw(width as usize)))
}

#[test]
fn test_fsm_types() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let inst_ty: TypeHandle = InstanceType::get(&ctx).into();
    let xdsl_inst_ty: TypeHandle = InstanceTypeType::get(&ctx).into();
    let clock_ty: TypeHandle = ClockType::get(&ctx).into();
    let i1_ty: TypeHandle = IntegerType::get(&ctx, 1, Signedness::Signless).into();

    assert!(is_instance_type(&ctx, inst_ty));
    assert!(is_instance_type(&ctx, xdsl_inst_ty));
    assert!(!is_instance_type(&ctx, clock_ty));
    assert!(!is_instance_type(&ctx, i1_ty));
}

#[test]
fn test_fsm_machine_creation_and_structure() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let i1_ty: TypeHandle = IntegerType::get(&ctx, 1, Signedness::Signless).into();
    let i16_ty: TypeHandle = IntegerType::get(&ctx, 16, Signedness::Signless).into();

    // Create an FSM: in %arg0: i1, %arg1: i16 -> out %res: i16, initial state "IDLE"
    let machine = MachineOp::new(
        &mut ctx,
        "traffic_light",
        "IDLE",
        vec![i1_ty, i16_ty],
        vec![i16_ty],
    );
    let body = machine.get_body(&ctx);

    // Variable "cnt" : i16
    let cnt_init = int_attr(&mut ctx, 16, 0);
    let var_cnt = VariableOp::new(&mut ctx, "cnt", Box::new(cnt_init), i16_ty);
    var_cnt.get_operation().insert_at_back(body, &ctx);
    let cnt_val = var_cnt.result(&ctx);

    // State "IDLE"
    let idle_state = StateOp::new(&mut ctx, "IDLE");
    idle_state.get_operation().insert_at_back(body, &ctx);

    let idle_out_block = idle_state.ensure_output_block(&mut ctx);
    let idle_out = OutputOp::new(&mut ctx, vec![cnt_val]);
    idle_out
        .get_operation()
        .insert_at_back(idle_out_block, &ctx);

    let idle_trans_block = idle_state.ensure_transitions_block(&mut ctx);
    let trans_idle_to_busy = TransitionOp::new(&mut ctx, "BUSY");
    trans_idle_to_busy
        .get_operation()
        .insert_at_back(idle_trans_block, &ctx);

    // State "BUSY"
    let busy_state = StateOp::new(&mut ctx, "BUSY");
    busy_state.get_operation().insert_at_back(body, &ctx);

    let busy_out_block = busy_state.ensure_output_block(&mut ctx);
    let busy_out = OutputOp::new(&mut ctx, vec![cnt_val]);
    busy_out
        .get_operation()
        .insert_at_back(busy_out_block, &ctx);

    let busy_trans_block = busy_state.ensure_transitions_block(&mut ctx);
    let trans_busy_to_idle = TransitionOp::new(&mut ctx, "IDLE");
    trans_busy_to_idle
        .get_operation()
        .insert_at_back(busy_trans_block, &ctx);

    // Verify machine
    verify_op(&machine, &ctx).expect("machine should verify");

    // Inspect accessors
    assert_eq!(machine.machine_name(&ctx), "traffic_light");
    assert_eq!(machine.initial_state(&ctx), "IDLE");
    assert_eq!(machine.num_inputs(&ctx), 2);
    assert_eq!(machine.input_types(&ctx), vec![i1_ty, i16_ty]);
    assert_eq!(machine.result_types(&ctx), vec![i16_ty]);
    assert_eq!(machine.states(&ctx).len(), 2);
    assert!(machine.get_state(&ctx, "IDLE").is_some());
    assert!(machine.get_state(&ctx, "BUSY").is_some());
    assert!(machine.get_state(&ctx, "NON_EXISTENT").is_none());
    assert!(machine.get_variable(&ctx, "cnt").is_some());
}

#[test]
fn test_fsm_guards_actions_and_updates() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let i1_ty: TypeHandle = IntegerType::get(&ctx, 1, Signedness::Signless).into();
    let i16_ty: TypeHandle = IntegerType::get(&ctx, 16, Signedness::Signless).into();

    let machine = MachineOp::new(&mut ctx, "guarded_fsm", "S0", vec![i1_ty], vec![i1_ty]);
    let body = machine.get_body(&ctx);
    let in_flag = machine.get_input(&ctx, 0);

    let v0_init = int_attr(&mut ctx, 16, 100);
    let var0 = VariableOp::new(&mut ctx, "v0", Box::new(v0_init), i16_ty);
    var0.get_operation().insert_at_back(body, &ctx);

    let s0 = StateOp::new(&mut ctx, "S0");
    s0.get_operation().insert_at_back(body, &ctx);

    let s0_out_block = s0.ensure_output_block(&mut ctx);
    let s0_out = OutputOp::new(&mut ctx, vec![in_flag]);
    s0_out.get_operation().insert_at_back(s0_out_block, &ctx);

    let s0_trans_block = s0.ensure_transitions_block(&mut ctx);
    let trans = TransitionOp::new(&mut ctx, "S0");

    // Guard: returns in_flag (i1)
    let guard_block = trans.ensure_guard_block(&mut ctx);
    let ret_op = ReturnOp::new(&mut ctx, Some(in_flag));
    ret_op.get_operation().insert_at_back(guard_block, &ctx);

    // Action: updates v0 with new value
    let action_block = trans.ensure_action_block(&mut ctx);
    let v0_res = var0.result(&ctx);
    let update = UpdateOp::new(&mut ctx, v0_res, v0_res);
    update.get_operation().insert_at_back(action_block, &ctx);

    trans.get_operation().insert_at_back(s0_trans_block, &ctx);

    assert!(trans.has_guard(&ctx));
    assert!(trans.has_action(&ctx));
    assert_eq!(trans.updates(&ctx).len(), 1);

    verify_op(&machine, &ctx).expect("guarded FSM should verify");
}

#[test]
fn test_fsm_sw_instance_and_trigger() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let i1_ty: TypeHandle = IntegerType::get(&ctx, 1, Signedness::Signless).into();
    let inst_ty: TypeHandle = InstanceType::get(&ctx).into();

    let root_mod = ModuleOp::new(&mut ctx, "top".try_into().unwrap(), vec![i1_ty]);
    let mod_body = root_mod.get_body(&ctx);
    let in_sig = root_mod.get_input(&ctx, 0);

    // Define machine inside top
    let machine = MachineOp::new(&mut ctx, "counter", "A", vec![i1_ty], vec![i1_ty]);
    let m_body = machine.get_body(&ctx);
    let st_a = StateOp::new(&mut ctx, "A");
    st_a.get_operation().insert_at_back(m_body, &ctx);
    let st_a_out_block = st_a.ensure_output_block(&mut ctx);
    let out = OutputOp::new(&mut ctx, vec![in_sig]);
    out.get_operation().insert_at_back(st_a_out_block, &ctx);

    let st_a_trans = st_a.ensure_transitions_block(&mut ctx);
    let tr = TransitionOp::new(&mut ctx, "A");
    tr.get_operation().insert_at_back(st_a_trans, &ctx);

    machine.get_operation().insert_at_back(mod_body, &ctx);

    // Instantiate with fsm.instance
    let instance = InstanceOp::new(&mut ctx, "c_inst", "counter", inst_ty);
    instance.get_operation().insert_at_back(mod_body, &ctx);
    let inst_handle = instance.result(&ctx);

    // Trigger with fsm.trigger
    let trigger = TriggerOp::new(&mut ctx, inst_handle, vec![in_sig], vec![i1_ty]);
    trigger.get_operation().insert_at_back(mod_body, &ctx);

    assert_eq!(trigger.inputs(&ctx).len(), 1);
    assert_eq!(trigger.outputs(&ctx).len(), 1);
    assert_eq!(trigger.instance(&ctx), inst_handle);

    verify_op(&root_mod, &ctx).expect("module with fsm.instance and trigger should verify");
}

#[test]
fn test_fsm_hw_instance() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let i1_ty: TypeHandle = IntegerType::get(&ctx, 1, Signedness::Signless).into();
    let clock_ty: TypeHandle = ClockType::get(&ctx).into();

    let root_mod = ModuleOp::new(
        &mut ctx,
        "hw_top".try_into().unwrap(),
        vec![i1_ty, clock_ty, i1_ty],
    );
    let mod_body = root_mod.get_body(&ctx);
    let in_sig = root_mod.get_input(&ctx, 0);
    let clk = root_mod.get_input(&ctx, 1);
    let rst = root_mod.get_input(&ctx, 2);

    let machine = MachineOp::new(&mut ctx, "hw_fsm", "ST0", vec![i1_ty], vec![i1_ty]);
    let m_body = machine.get_body(&ctx);
    let st0 = StateOp::new(&mut ctx, "ST0");
    st0.get_operation().insert_at_back(m_body, &ctx);
    let st0_out_block = st0.ensure_output_block(&mut ctx);
    let out = OutputOp::new(&mut ctx, vec![in_sig]);
    out.get_operation().insert_at_back(st0_out_block, &ctx);

    let st0_trans = st0.ensure_transitions_block(&mut ctx);
    let tr = TransitionOp::new(&mut ctx, "ST0");
    tr.get_operation().insert_at_back(st0_trans, &ctx);

    machine.get_operation().insert_at_back(mod_body, &ctx);

    // Instantiate with fsm.hw_instance
    let hw_inst = HWInstanceOp::new(
        &mut ctx,
        "hw_inst_0",
        "hw_fsm",
        vec![in_sig],
        clk,
        rst,
        vec![i1_ty],
    );
    hw_inst.get_operation().insert_at_back(mod_body, &ctx);

    assert_eq!(hw_inst.instance_name(&ctx), "hw_inst_0");
    assert_eq!(hw_inst.machine(&ctx), "hw_fsm");
    assert_eq!(hw_inst.clock(&ctx), clk);
    assert_eq!(hw_inst.reset(&ctx), rst);
    assert_eq!(hw_inst.inputs(&ctx).len(), 1);
    assert_eq!(hw_inst.outputs(&ctx).len(), 1);

    verify_op(&root_mod, &ctx).expect("hw_instance in module should verify");
}

#[test]
fn test_fsm_rejects_missing_initial_state() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    // Initial state "GHOST" does not exist in machine
    let machine = MachineOp::new(&mut ctx, "bad_fsm", "GHOST", vec![], vec![]);
    let body = machine.get_body(&ctx);
    let s0 = StateOp::new(&mut ctx, "REAL");
    s0.get_operation().insert_at_back(body, &ctx);

    let err = verify_op(&machine, &ctx).unwrap_err().to_string();
    assert!(
        err.contains("Can not find initial state"),
        "error should report missing initial state, got: {}",
        err
    );
}

#[test]
fn test_fsm_rejects_missing_next_state() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let machine = MachineOp::new(&mut ctx, "bad_trans_fsm", "S0", vec![], vec![]);
    let body = machine.get_body(&ctx);
    let s0 = StateOp::new(&mut ctx, "S0");
    s0.get_operation().insert_at_back(body, &ctx);

    let trans_block = s0.ensure_transitions_block(&mut ctx);
    // Transition points to non-existent state "NON_EXISTENT"
    let trans = TransitionOp::new(&mut ctx, "NON_EXISTENT");
    trans.get_operation().insert_at_back(trans_block, &ctx);

    let err = verify_op(&machine, &ctx).unwrap_err().to_string();
    assert!(
        err.contains("Can not find next state"),
        "error should report missing next state, got: {}",
        err
    );
}

#[test]
fn test_fsm_rejects_multiple_updates_to_same_variable() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let i16_ty: TypeHandle = IntegerType::get(&ctx, 16, Signedness::Signless).into();
    let machine = MachineOp::new(&mut ctx, "dup_update_fsm", "A", vec![], vec![]);
    let body = machine.get_body(&ctx);

    let v_init = int_attr(&mut ctx, 16, 0);
    let var = VariableOp::new(&mut ctx, "v", Box::new(v_init), i16_ty);
    var.get_operation().insert_at_back(body, &ctx);

    let state = StateOp::new(&mut ctx, "A");
    state.get_operation().insert_at_back(body, &ctx);

    let trans_block = state.ensure_transitions_block(&mut ctx);
    let trans = TransitionOp::new(&mut ctx, "A");
    let action_block = trans.ensure_action_block(&mut ctx);

    // Two updates targeting the same variable var.result()
    let v_res = var.result(&ctx);
    let u1 = UpdateOp::new(&mut ctx, v_res, v_res);
    let u2 = UpdateOp::new(&mut ctx, v_res, v_res);
    u1.get_operation().insert_at_back(action_block, &ctx);
    u2.get_operation().insert_at_back(action_block, &ctx);

    trans.get_operation().insert_at_back(trans_block, &ctx);

    let err = verify_op(&machine, &ctx).unwrap_err().to_string();
    assert!(
        err.contains(
            "Multiple updates to the same variable within a single action region is disallowed"
        ),
        "error should report disallowed multiple updates, got: {}",
        err
    );
}

#[test]
fn test_fsm_rejects_update_destination_not_variable() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let i16_ty: TypeHandle = IntegerType::get(&ctx, 16, Signedness::Signless).into();
    let machine = MachineOp::new(&mut ctx, "bad_dest_fsm", "A", vec![i16_ty], vec![]);
    let body = machine.get_body(&ctx);
    let arg0 = machine.get_input(&ctx, 0);

    let state = StateOp::new(&mut ctx, "A");
    state.get_operation().insert_at_back(body, &ctx);

    let trans_block = state.ensure_transitions_block(&mut ctx);
    let trans = TransitionOp::new(&mut ctx, "A");
    let action_block = trans.ensure_action_block(&mut ctx);

    // arg0 is a block argument, not defined by fsm.variable!
    let update = UpdateOp::new(&mut ctx, arg0, arg0);
    update.get_operation().insert_at_back(action_block, &ctx);
    trans.get_operation().insert_at_back(trans_block, &ctx);

    let err = verify_op(&machine, &ctx).unwrap_err().to_string();
    assert!(
        err.contains("Destination is not a variable operation"),
        "error should report destination not a variable, got: {}",
        err
    );
}

#[test]
fn test_fsm_canonicalization_and_reachability() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    // Machine with reachable states A -> B, and unreachable state ORPHAN
    let machine = MachineOp::new(&mut ctx, "reach_fsm", "A", vec![], vec![]);
    let body = machine.get_body(&ctx);

    let state_a = StateOp::new(&mut ctx, "A");
    state_a.get_operation().insert_at_back(body, &ctx);
    let a_trans_block = state_a.ensure_transitions_block(&mut ctx);
    let tr_a = TransitionOp::new(&mut ctx, "B");
    tr_a.get_operation().insert_at_back(a_trans_block, &ctx);

    let state_b = StateOp::new(&mut ctx, "B");
    state_b.get_operation().insert_at_back(body, &ctx);
    let b_trans_block = state_b.ensure_transitions_block(&mut ctx);
    let tr_b = TransitionOp::new(&mut ctx, "A");
    tr_b.get_operation().insert_at_back(b_trans_block, &ctx);

    let state_orphan = StateOp::new(&mut ctx, "ORPHAN");
    state_orphan.get_operation().insert_at_back(body, &ctx);

    verify_op(&machine, &ctx).expect("machine with orphan state is structurally valid");

    let reachable = compute_reachable_states(&ctx, &machine);
    assert_eq!(reachable.len(), 2);
    assert!(reachable.contains("A"));
    assert!(reachable.contains("B"));
    assert!(!reachable.contains("ORPHAN"));

    let unreachable = find_unreachable_states(&ctx, &machine);
    assert_eq!(unreachable, vec!["ORPHAN"]);

    let eliminated =
        eliminate_unreachable_states(&mut ctx, &machine).expect("elimination succeeds");
    assert_eq!(eliminated, 1);
    assert_eq!(machine.states(&ctx).len(), 2);
    assert!(machine.get_state(&ctx, "ORPHAN").is_none());
}

#[test]
fn test_fsm_lowering_plan() {
    let mut ctx = Context::new();
    register_all(&mut ctx);

    let i1_ty: TypeHandle = IntegerType::get(&ctx, 1, Signedness::Signless).into();
    let i8_ty: TypeHandle = IntegerType::get(&ctx, 8, Signedness::Signless).into();

    let machine = MachineOp::new(&mut ctx, "plan_fsm", "S0", vec![i1_ty], vec![i8_ty]);
    let body = machine.get_body(&ctx);

    for name in &["S0", "S1", "S2"] {
        let s = StateOp::new(&mut ctx, *name);
        s.get_operation().insert_at_back(body, &ctx);
        let out_block = s.ensure_output_block(&mut ctx);
        let val = int_attr(&mut ctx, 8, 42);
        let var = VariableOp::new(&mut ctx, format!("v_{}", name), Box::new(val), i8_ty);
        var.get_operation().insert_at_back(body, &ctx);
        let v_res = var.result(&ctx);
        let out = OutputOp::new(&mut ctx, vec![v_res]);
        out.get_operation().insert_at_back(out_block, &ctx);

        let trans_block = s.ensure_transitions_block(&mut ctx);
        let tr = TransitionOp::new(&mut ctx, "S0");
        tr.get_operation().insert_at_back(trans_block, &ctx);
    }

    let plan_bin =
        plan_fsm_lowering(&ctx, &machine, StateEncoding::Binary).expect("binary plan succeeds");
    assert_eq!(plan_bin.num_states, 3);
    assert_eq!(plan_bin.state_width, 2); // ceil(log2(3)) = 2 bits
    assert_eq!(plan_bin.state_map.get("S0"), Some(&0));
    assert_eq!(plan_bin.state_map.get("S1"), Some(&1));
    assert_eq!(plan_bin.state_map.get("S2"), Some(&2));

    let plan_onehot =
        plan_fsm_lowering(&ctx, &machine, StateEncoding::OneHot).expect("onehot plan succeeds");
    assert_eq!(plan_onehot.state_width, 3);
    assert_eq!(plan_onehot.state_map.get("S0"), Some(&1));
    assert_eq!(plan_onehot.state_map.get("S1"), Some(&2));
    assert_eq!(plan_onehot.state_map.get("S2"), Some(&4));
}

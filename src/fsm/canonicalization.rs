// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Canonicalization and analysis passes for the `fsm` dialect.
//!
//!(Canonicalization and Normal Forms):
//! - Unreachable state detection and elimination
//! - Redundant transition detection (after unconditional transitions)
//! - State graph reachability analysis

use std::collections::{HashSet, VecDeque};

use pliron::{context::Context, op::Op, operation::Operation, result::Result};

use super::ops::MachineOp;

/// Analyze state reachability starting from the initial state of an `fsm.machine`.
///
/// Returns a set of reachable state names.
pub fn compute_reachable_states(ctx: &Context, machine: &MachineOp) -> HashSet<String> {
    let mut reachable = HashSet::new();
    let initial_state = machine.initial_state(ctx);
    if initial_state.is_empty() {
        return reachable;
    }

    let mut queue = VecDeque::new();
    queue.push_back(initial_state.clone());
    reachable.insert(initial_state);

    while let Some(current_state_name) = queue.pop_front() {
        if let Some(state) = machine.get_state(ctx, &current_state_name) {
            for trans in state.transitions(ctx) {
                let next = trans.next_state(ctx);
                if !next.is_empty() && reachable.insert(next.clone()) {
                    queue.push_back(next);
                }
            }
        }
    }

    reachable
}

/// Check if an `fsm.machine` has any unreachable states.
pub fn find_unreachable_states(ctx: &Context, machine: &MachineOp) -> Vec<String> {
    let reachable = compute_reachable_states(ctx, machine);
    machine
        .states(ctx)
        .into_iter()
        .map(|s| s.state_name(ctx))
        .filter(|name| !reachable.contains(name))
        .collect()
}

/// Remove unreachable states from an `fsm.machine`.
///
/// Preserves semantic equivalence because unreachable states can never be entered
/// during machine execution.
pub fn eliminate_unreachable_states(ctx: &mut Context, machine: &MachineOp) -> Result<usize> {
    let unreachable = find_unreachable_states(ctx, machine);
    let count = unreachable.len();

    for name in unreachable {
        if let Some(state) = machine.get_state(ctx, &name) {
            let op_ptr = state.get_operation();
            Operation::erase(op_ptr, ctx);
        }
    }

    Ok(count)
}

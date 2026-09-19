// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Combinational cycle verification for `hw.module` graph regions.
//!
//! Enforces that no closed combinational dependency loops exist in a hardware module.
//! Sequential state operations (`seq.compreg`, `seq.firreg`, `seq.hlmem`) break combinational
//! paths by placing storage elements between cycles.

use std::collections::{HashMap, HashSet};

use crate::hw::ops::ModuleOp;
use pliron::{
    context::{Context, Ptr},
    linked_list::ContainsLinkedList,
    location::Located,
    operation::Operation,
    result::Result,
    verify_err,
};

#[allow(dead_code)]
/// Node visit state for cycle detection.
#[derive(Clone, Copy, PartialEq, Eq)]
enum VisitState {
    Visiting,
    Visited,
}

#[allow(dead_code)]
/// Returns `true` if the given operation is purely combinational.
///
/// Combinational operations have zero clock latency and pass signals combinationally
/// from inputs to outputs without an intervening sequential state boundary.
pub fn is_combinational_op(ctx: &Context, op_ptr: Ptr<Operation>) -> bool {
    // let name = op_ptr.deref(ctx).get_opid().name;
    let name = Operation::get_opid(op_ptr, ctx).name;
    let name_str: &str = name.as_ref();

    // All comb dialect operations are combinational
    if name_str.starts_with("comb.") {
        return true;
    }

    // Combinational data-routing operations in hw
    matches!(
        name_str,
        "hw.concat"
            | "hw.slice"
            | "hw.bitcast"
            | "hw.struct_create"
            | "hw.struct_extract"
            | "hw.union_create"
            | "hw.union_extract"
            | "hw.array_create"
            | "hw.array_slice"
            | "hw.array_get"
    )
}

#[allow(dead_code)]
/// Verify that no combinational cycles exist within the body of an `hw.module`.
///
/// Returns an error pointing to the cyclic operation if a closed combinational loop is detected.
pub fn check_comb_cycles(ctx: &Context, module: &ModuleOp) -> Result<()> {
    let body_block = module.get_body(ctx);
    let mut comb_ops: Vec<Ptr<Operation>> = Vec::new();
    let mut op_set: HashSet<Ptr<Operation>> = HashSet::new();

    for op_ptr in body_block.deref(ctx).iter(ctx) {
        if is_combinational_op(ctx, op_ptr) {
            comb_ops.push(op_ptr);
            op_set.insert(op_ptr);
        }
    }

    // Build directed dependency adjacency: B -> A means A depends on B (B produces value used by A)
    let mut state_map: HashMap<Ptr<Operation>, VisitState> = HashMap::new();

    for &start_op in &comb_ops {
        if state_map.get(&start_op).is_none() {
            dfs_check_cycles(ctx, start_op, &op_set, &mut state_map)?;
        }
    }

    Ok(())
}

#[allow(dead_code)]
fn dfs_check_cycles(
    ctx: &Context,
    current: Ptr<Operation>,
    comb_set: &HashSet<Ptr<Operation>>,
    state: &mut HashMap<Ptr<Operation>, VisitState>,
) -> Result<()> {
    state.insert(current, VisitState::Visiting);

    let op = current.deref(ctx);
    let op_name = Operation::get_opid(current, ctx).name;
    let op_name_str: &str = op_name.as_ref();
    for opd in op.operands() {
        if let Some(def_op) = opd.defining_op() {
            if comb_set.contains(&def_op) {
                match state.get(&def_op) {
                    Some(VisitState::Visiting) => {
                        return verify_err!(
                            op.loc(),
                            "combinational cycle detected involving operation '{}'",
                            op_name_str
                        );
                    }
                    Some(VisitState::Visited) => {
                        // Already checked and acyclic
                    }
                    None => {
                        dfs_check_cycles(ctx, def_op, comb_set, state)?;
                    }
                }
            }
        }
    }

    state.insert(current, VisitState::Visited);
    Ok(())
}

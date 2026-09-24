// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Lowering contracts and target mapping for the `fsm` dialect.
//!
//! Lowering path:
//! ```text
//! fsm.machine / fsm.hw_instance
//!         ↓
//!    hw.module + seq.firreg + comb.mux
//!         ↓
//!    sv.always_ff + sv.case
//! ```
//!
//! Semantics preserved during lowering:
//! - State register bitwidth: $\lceil \log_2(N) \rceil$ for binary encoding, or $N$ for one-hot.
//! - State transition priorities: earlier transitions in the transitions block have precedence.
//! - Variable persistence: state variables are lowered to synchronous registers updated only on active transition actions.
//! - Outputs: combinational mux trees selecting output values based on the current state register.

use std::collections::HashMap;

use pliron::{context::Context, location::Located, op::Op, result::Result, verify_err};

use super::ops::MachineOp;

/// State encoding strategy for hardware synthesis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateEncoding {
    /// Dense binary encoding using $\lceil \log_2(N) \rceil$ bits.
    Binary,
    /// One-hot encoding using $N$ bits (one flip-flop per state).
    OneHot,
}

/// Hardware lowering plan for an `fsm.machine`.
#[derive(Debug, Clone)]
pub struct FsmLoweringPlan {
    /// Machine symbol name.
    pub machine_name: String,
    /// Initial state name.
    pub initial_state: String,
    /// Number of states.
    pub num_states: usize,
    /// Bitwidth of the state register.
    pub state_width: u32,
    /// State encoding strategy.
    pub encoding: StateEncoding,
    /// Mapping from state name to encoded integer value.
    pub state_map: HashMap<String, u64>,
    /// Number of internal variables.
    pub num_variables: usize,
    /// Number of input ports.
    pub num_inputs: usize,
    /// Number of output ports.
    pub num_outputs: usize,
}

/// Compute the hardware lowering plan for an `fsm.machine`.
pub fn plan_fsm_lowering(
    ctx: &Context,
    machine: &MachineOp,
    encoding: StateEncoding,
) -> Result<FsmLoweringPlan> {
    let states = machine.states(ctx);
    let num_states = states.len();
    if num_states == 0 {
        return verify_err!(
            machine.get_operation().deref(ctx).loc(),
            "cannot lower fsm.machine with zero states"
        );
    }

    let state_width = match encoding {
        StateEncoding::Binary => {
            if num_states <= 1 {
                1
            } else {
                (num_states as f64).log2().ceil() as u32
            }
        }
        StateEncoding::OneHot => num_states as u32,
    };

    let mut state_map = HashMap::new();
    for (idx, state) in states.iter().enumerate() {
        let code = match encoding {
            StateEncoding::Binary => idx as u64,
            StateEncoding::OneHot => 1u64 << idx,
        };
        state_map.insert(state.state_name(ctx), code);
    }

    Ok(FsmLoweringPlan {
        machine_name: machine.machine_name(ctx),
        initial_state: machine.initial_state(ctx),
        num_states,
        state_width,
        encoding,
        state_map,
        num_variables: machine.variables(ctx).len(),
        num_inputs: machine.num_inputs(ctx),
        num_outputs: machine.result_types(ctx).len(),
    })
}

// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Operations for the `fsm` dialect.
//!
//! Models finite state machines with explicit states, transitions,
//! guards, actions, variables, and instantiations.

use std::collections::HashSet;

use pliron::{
    attribute::AttrObj,
    basic_block::BasicBlock,
    builtin::{
        attributes::{DictAttr, IdentifierAttr, StringAttr, TypeAttr, VecAttr},
        op_interfaces::{
            ATTR_KEY_SYM_NAME, IsTerminatorInterface, NOpdsInterface, NRegionsInterface,
            NResultsInterface, OneRegionInterface, OneResultInterface, SingleBlockRegionInterface,
            SymbolOpInterface,
        },
        type_interfaces::FunctionTypeInterface,
        types::{FunctionType, IntegerType, Signedness},
    },
    common_traits::Verify,
    context::{Context, Ptr},
    derive::pliron_op,
    identifier::Identifier,
    linked_list::ContainsLinkedList,
    location::Located,
    op::Op,
    operation::Operation,
    printable::Printable,
    region::Region,
    result::Result,
    r#type::{TypeHandle, Typed, TypedHandle},
    value::Value,
    verify_err,
};

use super::types::is_instance_type;

/// Strip leading `@` from a symbol reference string.
pub fn clean_symbol_ref(s: &str) -> &str {
    s.trim_start_matches('@')
}

/// Retrieve the symbol name or `"sym_name"` / `"name"` attribute of an operation.
pub fn get_op_symbol_or_name(op: &Operation, _ctx: &Context) -> Option<String> {
    if let Some(id_attr) = op.attributes.get::<IdentifierAttr>(&ATTR_KEY_SYM_NAME) {
        return Some(id_attr.as_ref().to_string());
    }
    if let Ok(key) = Identifier::try_from("sym_name") {
        if let Some(str_attr) = op.attributes.get::<StringAttr>(&key) {
            return Some(clean_symbol_ref(str_attr.as_ref()).to_string());
        }
        if let Some(id_attr) = op.attributes.get::<IdentifierAttr>(&key) {
            return Some(clean_symbol_ref(id_attr.as_ref().as_ref()).to_string());
        }
    }
    if let Ok(key) = Identifier::try_from("name") {
        if let Some(str_attr) = op.attributes.get::<StringAttr>(&key) {
            return Some(clean_symbol_ref(str_attr.as_ref()).to_string());
        }
        if let Some(id_attr) = op.attributes.get::<IdentifierAttr>(&key) {
            return Some(clean_symbol_ref(id_attr.as_ref().as_ref()).to_string());
        }
    }
    None
}

/// Set both the pliron `ATTR_KEY_SYM_NAME` and `"sym_name"` attribute on an operation.
pub fn set_op_symbol_and_name(op: &mut Operation, name: &str) {
    let clean = clean_symbol_ref(name);
    if let Ok(id) = Identifier::try_from(clean) {
        op.attributes
            .set(ATTR_KEY_SYM_NAME.clone(), IdentifierAttr::new(id));
    }
    if let Ok(sym_key) = Identifier::try_from("sym_name") {
        op.attributes
            .set(sym_key, StringAttr::from(clean.to_string()));
    }
}

/// Look up an `fsm.machine` by name in the enclosing module or hierarchy.
pub fn find_machine(start_op: &Operation, ctx: &Context, target_name: &str) -> Option<MachineOp> {
    let target = clean_symbol_ref(target_name);
    let mut cur_parent = start_op.get_parent_op(ctx);
    while let Some(parent) = cur_parent {
        let parent_ref = parent.deref(ctx);
        for reg_idx in 0..parent_ref.num_regions() {
            let region = parent_ref.get_region(reg_idx);
            for block_ptr in region.deref(ctx).iter(ctx) {
                for inner_op in block_ptr.deref(ctx).iter(ctx) {
                    if let Some(machine) = Operation::get_op::<MachineOp>(inner_op, ctx) {
                        if clean_symbol_ref(&machine.machine_name(ctx)) == target {
                            return Some(machine);
                        }
                    }
                }
            }
        }
        cur_parent = parent_ref.get_parent_op(ctx);
    }
    None
}

/// Extract input and result types from a function type handle.
pub fn get_function_signature(
    ctx: &Context,
    ty: TypeHandle,
) -> Option<(Vec<TypeHandle>, Vec<TypeHandle>)> {
    let ty_ref = ty.deref(ctx);
    if let Some(ft) = ty_ref.downcast_ref::<FunctionType>() {
        return Some((ft.arg_types(), ft.res_types()));
    }
    if let Some(mt) = ty_ref.downcast_ref::<crate::hw::types::ModuleType>() {
        let inputs = mt.inputs().iter().map(|f| f.ty).collect();
        let outputs = mt.outputs().iter().map(|f| f.ty).collect();
        return Some((inputs, outputs));
    }
    None
}

// =========================================================================
// 1. MachineOp (fsm.machine)
// =========================================================================

/// Finite-state machine definition operation (`fsm.machine`).
///
/// Contains a single region body with states (`fsm.state`) and variables (`fsm.variable`).
/// Inputs to the state machine are bound to the entry block arguments.
#[pliron_op(
    name = "fsm.machine",
    format,
    interfaces = [
        OneRegionInterface,
        SingleBlockRegionInterface,
        SymbolOpInterface,
        NOpdsInterface<0>,
        NResultsInterface<0>,
    ],
)]
pub struct MachineOp;

impl Verify for MachineOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);

        // 1. Symbol name
        let name = match get_op_symbol_or_name(&op, ctx) {
            Some(n) if !n.is_empty() => n,
            _ => return verify_err!(op.loc(), "fsm.machine requires a non-empty symbol name"),
        };

        // 2. Initial state attribute
        let initial_state = match self.get_initial_state(ctx) {
            Some(s) if !s.is_empty() => s,
            _ => return verify_err!(op.loc(), "fsm.machine requires an initialState attribute"),
        };

        // 3. Function type attribute
        let (input_types, result_types) = match self.get_function_signature(ctx) {
            Some(sig) => sig,
            None => {
                return verify_err!(
                    op.loc(),
                    "fsm.machine requires a valid function_type attribute"
                );
            }
        };

        // 4. Entry block arguments match function_type inputs
        let body = self.get_body(ctx);
        let block = body.deref(ctx);
        if block.get_num_arguments() != input_types.len() {
            return verify_err!(
                op.loc(),
                "fsm.machine entry block has {} arguments but function_type expects {}",
                block.get_num_arguments(),
                input_types.len()
            );
        }
        for (i, expected_ty) in input_types.iter().enumerate() {
            let actual_ty = block.get_argument(i).get_type(ctx);
            if actual_ty != *expected_ty {
                return verify_err!(
                    op.loc(),
                    "fsm.machine argument {} type mismatch: expected {}, got {}",
                    i,
                    expected_ty.disp(ctx),
                    actual_ty.disp(ctx)
                );
            }
        }

        // 5. Initial state exists in body
        let states = self.states(ctx);
        let mut state_names = HashSet::new();
        let mut initial_state_found = false;

        for state in &states {
            let s_name = state.state_name(ctx);
            if !state_names.insert(s_name.clone()) {
                return verify_err!(
                    op.loc(),
                    "duplicate state name '{}' in fsm.machine '{}'",
                    s_name,
                    name
                );
            }
            if s_name == initial_state {
                initial_state_found = true;
            }
        }

        if !initial_state_found {
            return verify_err!(
                op.loc(),
                "Can not find initial state: '{}' in machine '{}'",
                initial_state,
                name
            );
        }

        // 6. Check arg_names and arg_attrs consistency
        let get_attr_len = |attr_name: &str| -> Option<usize> {
            if let Ok(key) = Identifier::try_from(attr_name) {
                if let Some(v) = op.attributes.get::<VecAttr>(&key) {
                    return Some(v.0.len());
                }
                if let Some(d) = op.attributes.get::<DictAttr>(&key) {
                    return Some(d.0.0.len());
                }
            }
            None
        };

        let arg_names_len = get_attr_len("arg_names");
        let arg_attrs_len = get_attr_len("arg_attrs");
        if arg_names_len.is_some() != arg_attrs_len.is_some() {
            return verify_err!(op.loc(), "arg_attrs must be consistent with arg_names");
        }
        if let (Some(l1), Some(l2)) = (arg_names_len, arg_attrs_len) {
            if l1 != l2 {
                return verify_err!(
                    op.loc(),
                    "The number of arg_attrs and arg_names should be the same"
                );
            }
        }

        // 7. Check res_names and res_attrs consistency
        let res_names_len = get_attr_len("res_names");
        let res_attrs_len = get_attr_len("res_attrs");
        if res_names_len.is_some() != res_attrs_len.is_some() {
            return verify_err!(op.loc(), "res_attrs must be consistent with res_names");
        }
        if let (Some(l1), Some(l2)) = (res_names_len, res_attrs_len) {
            if l1 != l2 {
                return verify_err!(
                    op.loc(),
                    "The number of res_attrs and res_names should be the same"
                );
            }
        }

        // 8. If machine has results, every state must have an output region with OutputOp
        if !result_types.is_empty() {
            for state in &states {
                if state.output_op(ctx).is_none() {
                    return verify_err!(
                        state.get_operation().deref(ctx).loc(),
                        "State must have a non-empty output region when the machine has results"
                    );
                }
            }
        }

        Ok(())
    }
}

impl MachineOp {
    /// Create a new `fsm.machine` with name, initial state, and input/result types.
    pub fn new(
        ctx: &mut Context,
        name: impl Into<StringAttr>,
        initial_state: impl Into<StringAttr>,
        input_types: Vec<TypeHandle>,
        result_types: Vec<TypeHandle>,
    ) -> Self {
        let name_attr = name.into();
        let init_state_attr = initial_state.into();
        let fn_ty: TypeHandle =
            FunctionType::get(ctx, input_types.clone(), result_types.clone()).into();

        let op = Operation::new(ctx, Self::get_concrete_op_info(), vec![], vec![], vec![], 1);
        let machine = MachineOp { op };

        set_op_symbol_and_name(
            &mut machine.get_operation().deref_mut(ctx),
            name_attr.as_ref(),
        );
        if let Ok(key) = Identifier::try_from("initialState") {
            machine
                .get_operation()
                .deref_mut(ctx)
                .attributes
                .set(key, init_state_attr);
        }
        if let Ok(key) = Identifier::try_from("function_type") {
            machine
                .get_operation()
                .deref_mut(ctx)
                .attributes
                .set(key, TypeAttr::new(fn_ty));
        }

        // Initialize body block with arguments for inputs
        let region = machine.get_region(ctx);
        let block = BasicBlock::new(ctx, None, input_types);
        block.insert_at_front(region, ctx);

        machine
    }

    /// Create an `fsm.machine` using an existing `FunctionType` handle.
    pub fn new_with_function_type(
        ctx: &mut Context,
        name: impl Into<StringAttr>,
        initial_state: impl Into<StringAttr>,
        function_type: TypedHandle<FunctionType>,
    ) -> Self {
        let (arg_types, res_types) = {
            let ft = function_type.deref(ctx);
            (ft.arg_types(), ft.res_types())
        };
        Self::new(ctx, name, initial_state, arg_types, res_types)
    }

    /// Machine symbol name.
    pub fn machine_name(&self, ctx: &Context) -> String {
        get_op_symbol_or_name(&self.get_operation().deref(ctx), ctx)
            .unwrap_or_else(|| "unnamed_machine".to_string())
    }

    /// Initial state name.
    pub fn initial_state(&self, ctx: &Context) -> String {
        self.get_initial_state(ctx)
            .unwrap_or_else(|| "".to_string())
    }

    fn get_initial_state(&self, ctx: &Context) -> Option<String> {
        let op = self.get_operation().deref(ctx);
        if let Ok(key) = Identifier::try_from("initialState") {
            if let Some(s) = op.attributes.get::<StringAttr>(&key) {
                return Some(clean_symbol_ref(s.as_ref()).to_string());
            }
            if let Some(id) = op.attributes.get::<IdentifierAttr>(&key) {
                return Some(clean_symbol_ref(id.as_ref().as_ref()).to_string());
            }
        }
        None
    }

    /// Function signature (inputs, results).
    pub fn get_function_signature(
        &self,
        ctx: &Context,
    ) -> Option<(Vec<TypeHandle>, Vec<TypeHandle>)> {
        let op = self.get_operation().deref(ctx);
        if let Ok(key) = Identifier::try_from("function_type") {
            if let Some(t_attr) = op.attributes.get::<TypeAttr>(&key) {
                return get_function_signature(ctx, t_attr.get_type(ctx));
            }
        }
        None
    }

    /// Input types.
    pub fn input_types(&self, ctx: &Context) -> Vec<TypeHandle> {
        self.get_function_signature(ctx)
            .map(|(inputs, _)| inputs)
            .unwrap_or_default()
    }

    /// Result types.
    pub fn result_types(&self, ctx: &Context) -> Vec<TypeHandle> {
        self.get_function_signature(ctx)
            .map(|(_, results)| results)
            .unwrap_or_default()
    }

    /// Get entry body basic block.
    pub fn get_body(&self, ctx: &Context) -> Ptr<BasicBlock> {
        self.get_region(ctx)
            .deref(ctx)
            .get_entry_block()
            .expect("fsm.machine body block exists")
    }

    /// Get input SSA value by index.
    pub fn get_input(&self, ctx: &Context, idx: usize) -> Value {
        self.get_body(ctx).deref(ctx).get_argument(idx)
    }

    /// Number of inputs.
    pub fn num_inputs(&self, ctx: &Context) -> usize {
        self.get_body(ctx).deref(ctx).get_num_arguments()
    }

    /// All states defined in the machine.
    pub fn states(&self, ctx: &Context) -> Vec<StateOp> {
        let mut result = Vec::new();
        let body = self.get_body(ctx);
        for op_ptr in body.deref(ctx).iter(ctx) {
            if let Some(state) = Operation::get_op::<StateOp>(op_ptr, ctx) {
                result.push(state);
            }
        }
        result
    }

    /// Find a state by name.
    pub fn get_state(&self, ctx: &Context, name: &str) -> Option<StateOp> {
        let target = clean_symbol_ref(name);
        self.states(ctx)
            .into_iter()
            .find(|s| clean_symbol_ref(&s.state_name(ctx)) == target)
    }

    /// All variables declared in the machine.
    pub fn variables(&self, ctx: &Context) -> Vec<VariableOp> {
        let mut result = Vec::new();
        let body = self.get_body(ctx);
        for op_ptr in body.deref(ctx).iter(ctx) {
            if let Some(var) = Operation::get_op::<VariableOp>(op_ptr, ctx) {
                result.push(var);
            }
        }
        result
    }

    /// Find a variable by name.
    pub fn get_variable(&self, ctx: &Context, name: &str) -> Option<VariableOp> {
        let target = clean_symbol_ref(name);
        self.variables(ctx)
            .into_iter()
            .find(|v| clean_symbol_ref(&v.variable_name(ctx)) == target)
    }
}

// =========================================================================
// 2. StateOp (fsm.state)
// =========================================================================

/// State definition operation (`fsm.state`).
///
/// Contains two regions:
/// - Region 0: `output` region, terminated by `fsm.output`.
/// - Region 1: `transitions` region, containing `fsm.transition` ops.
#[pliron_op(
    name = "fsm.state",
    format,
    interfaces = [
        NRegionsInterface<2>,
        NOpdsInterface<0>,
        NResultsInterface<0>,
        SymbolOpInterface,
    ],
)]
pub struct StateOp;

impl Verify for StateOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);

        // 1. Symbol name
        let name = match get_op_symbol_or_name(&op, ctx) {
            Some(n) if !n.is_empty() => n,
            _ => return verify_err!(op.loc(), "fsm.state requires a non-empty symbol name"),
        };

        // 2. Must be enclosed in an fsm.machine
        let parent_machine = op
            .get_parent_op(ctx)
            .and_then(|p| Operation::get_op::<MachineOp>(p, ctx));

        // 3. Region 0: Output region checks
        let output_reg = self.output_region(ctx);
        if let Some(out_block_ptr) = output_reg.deref(ctx).get_entry_block() {
            let out_block = out_block_ptr.deref(ctx);
            for inner_op_ptr in out_block.iter(ctx) {
                if Operation::get_op::<TransitionOp>(inner_op_ptr, ctx).is_some() {
                    return verify_err!(
                        inner_op_ptr.deref(ctx).loc(),
                        "Transition must be located in a transitions region"
                    );
                }
            }
            if out_block.get_head().is_some() && self.output_op(ctx).is_none() {
                return verify_err!(
                    op.loc(),
                    "fsm.state '{}' output region must terminate with an fsm.output",
                    name
                );
            }
        }

        // 4. Region 1: Transitions region checks
        let trans_reg = self.transitions_region(ctx);
        if let Some(trans_block_ptr) = trans_reg.deref(ctx).get_entry_block() {
            let trans_block = trans_block_ptr.deref(ctx);
            for inner_op_ptr in trans_block.iter(ctx) {
                if Operation::get_op::<OutputOp>(inner_op_ptr, ctx).is_some() {
                    return verify_err!(
                        inner_op_ptr.deref(ctx).loc(),
                        "Transition regions should not output any value"
                    );
                }
            }
        }

        // 5. If parent machine has results, output region must not be empty and must terminate with OutputOp
        if let Some(machine) = parent_machine {
            if !machine.result_types(ctx).is_empty() && self.output_op(ctx).is_none() {
                return verify_err!(
                    op.loc(),
                    "State must have a non-empty output region when the machine has results"
                );
            }
        }

        Ok(())
    }
}

impl StateOp {
    /// Create a new `fsm.state` with a name.
    pub fn new(ctx: &mut Context, name: impl Into<StringAttr>) -> Self {
        let name_attr = name.into();
        let op = Operation::new(ctx, Self::get_concrete_op_info(), vec![], vec![], vec![], 2);
        let state = StateOp { op };
        set_op_symbol_and_name(
            &mut state.get_operation().deref_mut(ctx),
            name_attr.as_ref(),
        );
        state
    }

    /// State symbol name.
    pub fn state_name(&self, ctx: &Context) -> String {
        get_op_symbol_or_name(&self.get_operation().deref(ctx), ctx)
            .unwrap_or_else(|| "unnamed_state".to_string())
    }

    /// Output region (Region 0).
    pub fn output_region(&self, ctx: &Context) -> Ptr<Region> {
        self.get_operation().deref(ctx).get_region(0)
    }

    /// Transitions region (Region 1).
    pub fn transitions_region(&self, ctx: &Context) -> Ptr<Region> {
        self.get_operation().deref(ctx).get_region(1)
    }

    /// Ensure that the output region has a basic block and return it.
    pub fn ensure_output_block(&self, ctx: &mut Context) -> Ptr<BasicBlock> {
        let reg = self.output_region(ctx);
        if let Some(b) = reg.deref(ctx).get_entry_block() {
            b
        } else {
            let block = BasicBlock::new(ctx, None, vec![]);
            block.insert_at_front(reg, ctx);
            block
        }
    }

    /// Ensure that the transitions region has a basic block and return it.
    pub fn ensure_transitions_block(&self, ctx: &mut Context) -> Ptr<BasicBlock> {
        let reg = self.transitions_region(ctx);
        if let Some(b) = reg.deref(ctx).get_entry_block() {
            b
        } else {
            let block = BasicBlock::new(ctx, None, vec![]);
            block.insert_at_front(reg, ctx);
            block
        }
    }

    /// Get the `fsm.output` operation in the output region if present.
    pub fn output_op(&self, ctx: &Context) -> Option<OutputOp> {
        let reg = self.output_region(ctx);
        let block_ptr = reg.deref(ctx).get_entry_block()?;
        let block = block_ptr.deref(ctx);
        for op_ptr in block.iter(ctx) {
            if let Some(out) = Operation::get_op::<OutputOp>(op_ptr, ctx) {
                return Some(out);
            }
        }
        None
    }

    /// Get all transitions outgoing from this state.
    pub fn transitions(&self, ctx: &Context) -> Vec<TransitionOp> {
        let mut result = Vec::new();
        let reg = self.transitions_region(ctx);
        if let Some(block_ptr) = reg.deref(ctx).get_entry_block() {
            let block = block_ptr.deref(ctx);
            for op_ptr in block.iter(ctx) {
                if let Some(tr) = Operation::get_op::<TransitionOp>(op_ptr, ctx) {
                    result.push(tr);
                }
            }
        }
        result
    }
}

// =========================================================================
// 3. OutputOp (fsm.output)
// =========================================================================

/// State output terminator operation (`fsm.output`).
///
/// Drives the values produced by the finite state machine while in this state.
#[pliron_op(
    name = "fsm.output",
    format,
    interfaces = [IsTerminatorInterface, NRegionsInterface<0>, NResultsInterface<0>],
)]
pub struct OutputOp;

impl Verify for OutputOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);

        let parent_op_ptr = match op.get_parent_op(ctx) {
            Some(p) => p,
            None => return Ok(()),
        };

        let parent_state = match Operation::get_op::<StateOp>(parent_op_ptr, ctx) {
            Some(s) => s,
            None => return verify_err!(op.loc(), "fsm.output parent must be an fsm.state"),
        };

        // Output must NOT be in the transitions region
        if let Some(p_reg) = op.get_parent_region(ctx) {
            if p_reg == parent_state.transitions_region(ctx) {
                return verify_err!(op.loc(), "Transition regions should not output any value");
            }
        }

        // Consistency with enclosing machine result types
        let state_op = parent_state.get_operation().deref(ctx);
        if let Some(machine_ptr) = state_op.get_parent_op(ctx) {
            if let Some(machine) = Operation::get_op::<MachineOp>(machine_ptr, ctx) {
                let expected_types = machine.result_types(ctx);
                let num_operands = op.get_num_operands();

                if num_operands != expected_types.len() {
                    return verify_err!(
                        op.loc(),
                        "OutputOp output type must be consistent with the machine \"{}\"",
                        machine.machine_name(ctx)
                    );
                }

                for i in 0..num_operands {
                    if op.get_operand(i).get_type(ctx) != expected_types[i] {
                        return verify_err!(
                            op.loc(),
                            "OutputOp output type must be consistent with the machine \"{}\"",
                            machine.machine_name(ctx)
                        );
                    }
                }
            }
        }

        Ok(())
    }
}

impl OutputOp {
    /// Create a new `fsm.output` with output values.
    pub fn new(ctx: &mut Context, outputs: Vec<Value>) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![],
            outputs,
            vec![],
            0,
        );
        OutputOp { op }
    }

    /// Driven output values.
    pub fn outputs(&self, ctx: &Context) -> Vec<Value> {
        let op = self.get_operation().deref(ctx);
        (0..op.get_num_operands())
            .map(|i| op.get_operand(i))
            .collect()
    }

    /// Number of driven output values.
    pub fn num_outputs(&self, ctx: &Context) -> usize {
        self.get_operation().deref(ctx).get_num_operands()
    }
}

// =========================================================================
// 4. TransitionOp (fsm.transition)
// =========================================================================

/// State transition definition operation (`fsm.transition`).
///
/// Contains two regions:
/// - Region 0: `guard` region (optional), terminated with `fsm.return`.
/// - Region 1: `action` region (optional), containing `fsm.update` operations.
#[pliron_op(
    name = "fsm.transition",
    format,
    interfaces = [NRegionsInterface<2>, NOpdsInterface<0>, NResultsInterface<0>],
)]
pub struct TransitionOp;

impl Verify for TransitionOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);

        // 1. Must be in transitions region of StateOp
        let parent_op_ptr = match op.get_parent_op(ctx) {
            Some(p) => p,
            None => return Ok(()),
        };
        let parent_state = match Operation::get_op::<StateOp>(parent_op_ptr, ctx) {
            Some(s) => s,
            None => {
                return verify_err!(
                    op.loc(),
                    "Transition must be located in a transitions region"
                );
            }
        };

        if let Some(p_reg) = op.get_parent_region(ctx) {
            if p_reg != parent_state.transitions_region(ctx) {
                return verify_err!(
                    op.loc(),
                    "Transition must be located in a transitions region"
                );
            }
        }

        // 2. nextState attribute
        let next_state = match self.get_next_state_name(ctx) {
            Some(s) if !s.is_empty() => s,
            _ => return verify_err!(op.loc(), "fsm.transition requires nextState attribute"),
        };

        // 3. Target state exists in the machine
        let state_op = parent_state.get_operation().deref(ctx);
        if let Some(machine_ptr) = state_op.get_parent_op(ctx) {
            if let Some(machine) = Operation::get_op::<MachineOp>(machine_ptr, ctx) {
                if machine.get_state(ctx, &next_state).is_none() {
                    return verify_err!(op.loc(), "Can not find next state: '{}'", next_state);
                }
            }
        }

        // 4. Guard region must terminate with ReturnOp
        let guard_reg = self.guard_region(ctx);
        if let Some(guard_block_ptr) = guard_reg.deref(ctx).get_entry_block() {
            let guard_block = guard_block_ptr.deref(ctx);
            if guard_block.get_head().is_some() {
                let term = guard_block.get_terminator(ctx);
                let is_return = term
                    .and_then(|t| Operation::get_op::<ReturnOp>(t, ctx))
                    .is_some();
                if !is_return {
                    return verify_err!(op.loc(), "Guard region must terminate with ReturnOp");
                }
            }
        }

        // 5. Multiple updates to the same variable within a single action region is disallowed
        let action_reg = self.action_region(ctx);
        if let Some(action_block_ptr) = action_reg.deref(ctx).get_entry_block() {
            let action_block = action_block_ptr.deref(ctx);
            let mut updated_vars = HashSet::new();

            for inner_op_ptr in action_block.iter(ctx) {
                if let Some(update) = Operation::get_op::<UpdateOp>(inner_op_ptr, ctx) {
                    let var = update.variable(ctx);
                    if !updated_vars.insert(var) {
                        return verify_err!(
                            inner_op_ptr.deref(ctx).loc(),
                            "Multiple updates to the same variable within a single action region is disallowed"
                        );
                    }
                }
                if Operation::get_op::<OutputOp>(inner_op_ptr, ctx).is_some() {
                    return verify_err!(
                        inner_op_ptr.deref(ctx).loc(),
                        "Transition regions should not output any value"
                    );
                }
            }
        }

        Ok(())
    }
}

impl TransitionOp {
    /// Create a new `fsm.transition` targeting `next_state`.
    pub fn new(ctx: &mut Context, next_state: impl Into<StringAttr>) -> Self {
        let target_attr = next_state.into();
        let op = Operation::new(ctx, Self::get_concrete_op_info(), vec![], vec![], vec![], 2);
        let tr = TransitionOp { op };

        if let Ok(key) = Identifier::try_from("nextState") {
            tr.get_operation()
                .deref_mut(ctx)
                .attributes
                .set(key, target_attr);
        }
        tr
    }

    /// Next state name.
    pub fn next_state(&self, ctx: &Context) -> String {
        self.get_next_state_name(ctx).unwrap_or_default()
    }

    fn get_next_state_name(&self, ctx: &Context) -> Option<String> {
        let op = self.get_operation().deref(ctx);
        if let Ok(key) = Identifier::try_from("nextState") {
            if let Some(s) = op.attributes.get::<StringAttr>(&key) {
                return Some(clean_symbol_ref(s.as_ref()).to_string());
            }
            if let Some(id) = op.attributes.get::<IdentifierAttr>(&key) {
                return Some(clean_symbol_ref(id.as_ref().as_ref()).to_string());
            }
        }
        None
    }

    /// Guard region (Region 0).
    pub fn guard_region(&self, ctx: &Context) -> Ptr<Region> {
        self.get_operation().deref(ctx).get_region(0)
    }

    /// Action region (Region 1).
    pub fn action_region(&self, ctx: &Context) -> Ptr<Region> {
        self.get_operation().deref(ctx).get_region(1)
    }

    /// Ensure guard region has a basic block and return it.
    pub fn ensure_guard_block(&self, ctx: &mut Context) -> Ptr<BasicBlock> {
        let reg = self.guard_region(ctx);
        if let Some(b) = reg.deref(ctx).get_entry_block() {
            b
        } else {
            let block = BasicBlock::new(ctx, None, vec![]);
            block.insert_at_front(reg, ctx);
            block
        }
    }

    /// Ensure action region has a basic block and return it.
    pub fn ensure_action_block(&self, ctx: &mut Context) -> Ptr<BasicBlock> {
        let reg = self.action_region(ctx);
        if let Some(b) = reg.deref(ctx).get_entry_block() {
            b
        } else {
            let block = BasicBlock::new(ctx, None, vec![]);
            block.insert_at_front(reg, ctx);
            block
        }
    }

    /// Whether this transition has a guard.
    pub fn has_guard(&self, ctx: &Context) -> bool {
        self.guard_region(ctx)
            .deref(ctx)
            .get_entry_block()
            .map(|b| b.deref(ctx).get_head().is_some())
            .unwrap_or(false)
    }

    /// Whether this transition has an action block with operations.
    pub fn has_action(&self, ctx: &Context) -> bool {
        self.action_region(ctx)
            .deref(ctx)
            .get_entry_block()
            .map(|b| b.deref(ctx).get_head().is_some())
            .unwrap_or(false)
    }

    /// Get `fsm.return` op from guard region if present.
    pub fn guard_return_op(&self, ctx: &Context) -> Option<ReturnOp> {
        let block_ptr = self.guard_region(ctx).deref(ctx).get_entry_block()?;
        let term = block_ptr.deref(ctx).get_terminator(ctx)?;
        Operation::get_op::<ReturnOp>(term, ctx)
    }

    /// All update operations in the action region.
    pub fn updates(&self, ctx: &Context) -> Vec<UpdateOp> {
        let mut result = Vec::new();
        if let Some(block_ptr) = self.action_region(ctx).deref(ctx).get_entry_block() {
            let block = block_ptr.deref(ctx);
            for op_ptr in block.iter(ctx) {
                if let Some(up) = Operation::get_op::<UpdateOp>(op_ptr, ctx) {
                    result.push(up);
                }
            }
        }
        result
    }
}

// =========================================================================
// 5. ReturnOp (fsm.return)
// =========================================================================

/// Region terminator operation (`fsm.return`).
///
/// Terminates `guard` or `action` regions of an `fsm.transition`.
/// In a guard region, returns an `i1` Boolean condition value.
#[pliron_op(
    name = "fsm.return",
    format,
    interfaces = [IsTerminatorInterface, NRegionsInterface<0>, NResultsInterface<0>],
)]
pub struct ReturnOp;

impl Verify for ReturnOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);

        let parent_op_ptr = match op.get_parent_op(ctx) {
            Some(p) => p,
            None => return Ok(()),
        };

        if Operation::get_op::<TransitionOp>(parent_op_ptr, ctx).is_none() {
            return verify_err!(op.loc(), "fsm.return parent must be an fsm.transition");
        }

        // If an operand is provided, it must be i1
        if op.get_num_operands() > 0 {
            let i1_ty: TypeHandle = IntegerType::get(ctx, 1, Signedness::Signless).into();
            let opd_ty = op.get_operand(0).get_type(ctx);
            if opd_ty != i1_ty {
                return verify_err!(
                    op.loc(),
                    "fsm.return condition operand must be i1, got {}",
                    opd_ty.disp(ctx)
                );
            }
        }

        Ok(())
    }
}

impl ReturnOp {
    /// Create a new `fsm.return` operation, optionally returning a guard condition value.
    pub fn new(ctx: &mut Context, value: Option<Value>) -> Self {
        let operands = value.into_iter().collect();
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![],
            operands,
            vec![],
            0,
        );
        ReturnOp { op }
    }

    /// Get returned condition value if present.
    pub fn value(&self, ctx: &Context) -> Option<Value> {
        let op = self.get_operation().deref(ctx);
        if op.get_num_operands() > 0 {
            Some(op.get_operand(0))
        } else {
            None
        }
    }
}

// =========================================================================
// 6. VariableOp (fsm.variable)
// =========================================================================

/// State variable declaration operation (`fsm.variable`).
///
/// Declares persistent internal state with an initial value within an `fsm.machine`.
#[pliron_op(
    name = "fsm.variable",
    format,
    interfaces = [NRegionsInterface<0>, NOpdsInterface<0>, OneResultInterface],
)]
pub struct VariableOp;

impl Verify for VariableOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);

        let name = match get_op_symbol_or_name(&op, ctx) {
            Some(n) if !n.is_empty() => n,
            _ => return verify_err!(op.loc(), "fsm.variable requires a non-empty name"),
        };

        if let Ok(key) = Identifier::try_from("initValue") {
            if op.attributes.0.get(&key).is_none() {
                return verify_err!(
                    op.loc(),
                    "fsm.variable '{}' requires an initValue attribute",
                    name
                );
            }
        }

        Ok(())
    }
}

impl VariableOp {
    /// Create a new `fsm.variable`.
    pub fn new(
        ctx: &mut Context,
        name: impl Into<StringAttr>,
        init_value: AttrObj,
        ty: TypeHandle,
    ) -> Self {
        let name_attr = name.into();
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![ty],
            vec![],
            vec![],
            0,
        );
        let var = VariableOp { op };

        if let Ok(key) = Identifier::try_from("name") {
            var.get_operation()
                .deref_mut(ctx)
                .attributes
                .set(key, name_attr);
        }
        if let Ok(key) = Identifier::try_from("initValue") {
            var.get_operation()
                .deref_mut(ctx)
                .attributes
                .0
                .insert(key, init_value);
        }
        var
    }

    /// Variable name.
    pub fn variable_name(&self, ctx: &Context) -> String {
        get_op_symbol_or_name(&self.get_operation().deref(ctx), ctx).unwrap_or_default()
    }

    /// Stored variable SSA result.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

// =========================================================================
// 7. UpdateOp (fsm.update)
// =========================================================================

/// State variable update operation (`fsm.update`).
///
/// Modifies an `fsm.variable` during a transition in its action region.
#[pliron_op(
    name = "fsm.update",
    format,
    interfaces = [NRegionsInterface<0>, NOpdsInterface<2>, NResultsInterface<0>],
)]
pub struct UpdateOp;

impl Verify for UpdateOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);

        // 1. Must only be located in the action region of a transition
        let parent_op_ptr = match op.get_parent_op(ctx) {
            Some(p) => p,
            None => {
                return verify_err!(
                    op.loc(),
                    "Update must only be located in the action region of a transition"
                );
            }
        };

        let parent_trans = match Operation::get_op::<TransitionOp>(parent_op_ptr, ctx) {
            Some(t) => t,
            None => {
                return verify_err!(
                    op.loc(),
                    "Update must only be located in the action region of a transition"
                );
            }
        };

        let parent_reg = match op.get_parent_region(ctx) {
            Some(r) => r,
            None => {
                return verify_err!(
                    op.loc(),
                    "Update must only be located in the action region of a transition"
                );
            }
        };

        if parent_reg != parent_trans.action_region(ctx) {
            return verify_err!(
                op.loc(),
                "Update must only be located in the action region of a transition"
            );
        }

        // 2. Destination must be a variable operation
        let var_val = op.get_operand(0);
        let def_op_ptr = match var_val.defining_op() {
            Some(d) => d,
            None => return verify_err!(op.loc(), "Destination is not a variable operation"),
        };

        if Operation::get_op::<VariableOp>(def_op_ptr, ctx).is_none() {
            return verify_err!(op.loc(), "Destination is not a variable operation");
        }

        // 3. Variable and new value must have the same type
        let val_val = op.get_operand(1);
        if var_val.get_type(ctx) != val_val.get_type(ctx) {
            return verify_err!(
                op.loc(),
                "fsm.update variable and value must have the same type"
            );
        }

        Ok(())
    }
}

impl UpdateOp {
    /// Create a new `fsm.update`.
    pub fn new(ctx: &mut Context, variable: Value, value: Value) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![],
            vec![variable, value],
            vec![],
            0,
        );
        UpdateOp { op }
    }

    /// Variable being updated.
    pub fn variable(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(0)
    }

    /// Next value assigned to variable.
    pub fn value(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(1)
    }
}

// =========================================================================
// 8. InstanceOp (fsm.instance)
// =========================================================================

/// Software/generic finite-state machine instance (`fsm.instance`).
///
/// Produces an instance handle (`!fsm.instance`) that can be advanced via `fsm.trigger`.
#[pliron_op(
    name = "fsm.instance",
    format,
    interfaces = [NRegionsInterface<0>, NOpdsInterface<0>, OneResultInterface],
)]
pub struct InstanceOp;

impl Verify for InstanceOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);

        // 1. Result type must be instance type
        let res_ty = op.get_result(0).get_type(ctx);
        if !is_instance_type(ctx, res_ty) {
            return verify_err!(op.loc(), "The instance operand must be Instance");
        }

        // 2. Machine attribute
        let machine_name = match self.get_machine_name(ctx) {
            Some(m) if !m.is_empty() => m,
            _ => return verify_err!(op.loc(), "fsm.instance requires machine attribute"),
        };

        // 3. If in a module or parent hierarchy, verify machine exists
        if op.get_parent_op(ctx).is_some() && find_machine(&op, ctx, &machine_name).is_none() {
            return verify_err!(op.loc(), "Machine definition does not exist");
        }

        Ok(())
    }
}

impl InstanceOp {
    /// Create a new `fsm.instance`.
    pub fn new(
        ctx: &mut Context,
        name: impl Into<StringAttr>,
        machine: impl Into<StringAttr>,
        res_ty: TypeHandle,
    ) -> Self {
        let name_attr = name.into();
        let machine_attr = machine.into();
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![res_ty],
            vec![],
            vec![],
            0,
        );
        let inst = InstanceOp { op };

        set_op_symbol_and_name(&mut inst.get_operation().deref_mut(ctx), name_attr.as_ref());
        if let Ok(key) = Identifier::try_from("machine") {
            inst.get_operation()
                .deref_mut(ctx)
                .attributes
                .set(key, machine_attr);
        }
        inst
    }

    /// Instance name.
    pub fn instance_name(&self, ctx: &Context) -> String {
        get_op_symbol_or_name(&self.get_operation().deref(ctx), ctx).unwrap_or_default()
    }

    /// Referenced machine name.
    pub fn machine(&self, ctx: &Context) -> String {
        self.get_machine_name(ctx).unwrap_or_default()
    }

    fn get_machine_name(&self, ctx: &Context) -> Option<String> {
        let op = self.get_operation().deref(ctx);
        if let Ok(key) = Identifier::try_from("machine") {
            if let Some(s) = op.attributes.get::<StringAttr>(&key) {
                return Some(clean_symbol_ref(s.as_ref()).to_string());
            }
            if let Some(id) = op.attributes.get::<IdentifierAttr>(&key) {
                return Some(clean_symbol_ref(id.as_ref().as_ref()).to_string());
            }
        }
        None
    }

    /// Stored instance handle value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

// =========================================================================
// 9. TriggerOp (fsm.trigger)
// =========================================================================

/// State machine trigger operation (`fsm.trigger`).
///
/// Advances an `fsm.instance` given a list of input arguments, producing output results.
/// Operands are `(inputs..., instance)` where the last operand is the instance handle.
#[pliron_op(name = "fsm.trigger", format, interfaces = [NRegionsInterface<0>])]
pub struct TriggerOp;

impl Verify for TriggerOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);

        let num_opds = op.get_num_operands();
        if num_opds < 1 {
            return verify_err!(
                op.loc(),
                "fsm.trigger requires at least 1 operand (the instance)"
            );
        }

        // Instance is the last operand
        let inst_val = op.get_operand(num_opds - 1);
        if !is_instance_type(ctx, inst_val.get_type(ctx)) {
            return verify_err!(op.loc(), "The instance operand must be Instance");
        }

        // Check that defining entity is an InstanceOp if resolvable
        let inst_op_ptr = match inst_val.defining_op() {
            Some(d) => d,
            None => return verify_err!(op.loc(), "The instance operand must be Instance"),
        };
        let inst_op = match Operation::get_op::<InstanceOp>(inst_op_ptr, ctx) {
            Some(i) => i,
            None => return verify_err!(op.loc(), "The instance operand must be Instance"),
        };

        let machine_name = inst_op.machine(ctx);
        if let Some(machine) = find_machine(&op, ctx, &machine_name) {
            // Check input count and types
            let inputs_count = num_opds - 1;
            let expected_inputs = machine.input_types(ctx);
            if inputs_count != expected_inputs.len() {
                return verify_err!(
                    op.loc(),
                    "TriggerOp input types must be consistent with the machine \"{}\"",
                    machine_name
                );
            }
            for i in 0..inputs_count {
                if op.get_operand(i).get_type(ctx) != expected_inputs[i] {
                    return verify_err!(
                        op.loc(),
                        "TriggerOp input types must be consistent with the machine \"{}\"",
                        machine_name
                    );
                }
            }

            // Check output count and types
            let num_results = op.get_num_results();
            let expected_results = machine.result_types(ctx);
            if num_results != expected_results.len() {
                return verify_err!(
                    op.loc(),
                    "TriggerOp output types must be consistent with the machine \"{}\"",
                    machine_name
                );
            }
            for i in 0..num_results {
                if op.get_result(i).get_type(ctx) != expected_results[i] {
                    return verify_err!(
                        op.loc(),
                        "TriggerOp output types must be consistent with the machine \"{}\"",
                        machine_name
                    );
                }
            }
        } else if op.get_parent_op(ctx).is_some() {
            return verify_err!(op.loc(), "Machine definition does not exist");
        }

        Ok(())
    }
}

impl TriggerOp {
    /// Create a new `fsm.trigger` operation.
    pub fn new(
        ctx: &mut Context,
        instance: Value,
        inputs: Vec<Value>,
        output_types: Vec<TypeHandle>,
    ) -> Self {
        let mut operands = inputs;
        operands.push(instance);
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            output_types,
            operands,
            vec![],
            0,
        );
        TriggerOp { op }
    }

    /// Instance handle operand.
    pub fn instance(&self, ctx: &Context) -> Value {
        let op = self.get_operation().deref(ctx);
        op.get_operand(op.get_num_operands() - 1)
    }

    /// Inputs driven to state machine.
    pub fn inputs(&self, ctx: &Context) -> Vec<Value> {
        let op = self.get_operation().deref(ctx);
        let count = op.get_num_operands() - 1;
        (0..count).map(|i| op.get_operand(i)).collect()
    }

    /// Outputs produced by trigger step.
    pub fn outputs(&self, ctx: &Context) -> Vec<Value> {
        let op = self.get_operation().deref(ctx);
        (0..op.get_num_results())
            .map(|i| op.get_result(i))
            .collect()
    }
}

// =========================================================================
// 10. HWInstanceOp (fsm.hw_instance)
// =========================================================================

/// Hardware-style finite-state machine instance (`fsm.hw_instance`).
///
/// Instantiates an `fsm.machine` in an RTL context connected to a clock and reset.
/// Operands are `(inputs..., clock, reset)`.
#[pliron_op(name = "fsm.hw_instance", format, interfaces = [NRegionsInterface<0>])]
pub struct HWInstanceOp;

impl Verify for HWInstanceOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);

        let num_opds = op.get_num_operands();
        if num_opds < 2 {
            return verify_err!(
                op.loc(),
                "fsm.hw_instance requires at least clock and reset operands"
            );
        }

        // Machine attribute
        let machine_name = match self.get_machine_name(ctx) {
            Some(m) if !m.is_empty() => m,
            _ => return verify_err!(op.loc(), "fsm.hw_instance requires machine attribute"),
        };

        let inst_name = self.instance_name(ctx);

        // Verify machine existence and port signatures if machine can be resolved
        if let Some(machine) = find_machine(&op, ctx, &machine_name) {
            let num_inputs = num_opds - 2;
            let expected_inputs = machine.input_types(ctx);
            if num_inputs != expected_inputs.len() {
                return verify_err!(
                    op.loc(),
                    "HWInstanceOp \"{}\" input type must be consistent with the machine \"{}\"",
                    inst_name,
                    machine_name
                );
            }
            for i in 0..num_inputs {
                if op.get_operand(i).get_type(ctx) != expected_inputs[i] {
                    return verify_err!(
                        op.loc(),
                        "HWInstanceOp \"{}\" input type must be consistent with the machine \"{}\"",
                        inst_name,
                        machine_name
                    );
                }
            }

            let num_results = op.get_num_results();
            let expected_results = machine.result_types(ctx);
            if num_results != expected_results.len() {
                return verify_err!(
                    op.loc(),
                    "HWInstanceOp \"{}\" output type must be consistent with the machine \"{}\"",
                    inst_name,
                    machine_name
                );
            }
            for i in 0..num_results {
                if op.get_result(i).get_type(ctx) != expected_results[i] {
                    return verify_err!(
                        op.loc(),
                        "HWInstanceOp \"{}\" output type must be consistent with the machine \"{}\"",
                        inst_name,
                        machine_name
                    );
                }
            }
        } else if op.get_parent_op(ctx).is_some() {
            return verify_err!(op.loc(), "Machine definition does not exist");
        }

        Ok(())
    }
}

impl HWInstanceOp {
    /// Create a new `fsm.hw_instance`.
    pub fn new(
        ctx: &mut Context,
        name: impl Into<StringAttr>,
        machine: impl Into<StringAttr>,
        inputs: Vec<Value>,
        clock: Value,
        reset: Value,
        output_types: Vec<TypeHandle>,
    ) -> Self {
        let name_attr = name.into();
        let machine_attr = machine.into();

        let mut operands = inputs;
        operands.push(clock);
        operands.push(reset);

        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            output_types,
            operands,
            vec![],
            0,
        );
        let inst = HWInstanceOp { op };

        set_op_symbol_and_name(&mut inst.get_operation().deref_mut(ctx), name_attr.as_ref());
        if let Ok(key) = Identifier::try_from("machine") {
            inst.get_operation()
                .deref_mut(ctx)
                .attributes
                .set(key, machine_attr);
        }
        inst
    }

    /// Instance symbol/name.
    pub fn instance_name(&self, ctx: &Context) -> String {
        get_op_symbol_or_name(&self.get_operation().deref(ctx), ctx)
            .unwrap_or_else(|| "unnamed_hw_inst".to_string())
    }

    /// Referenced machine name.
    pub fn machine(&self, ctx: &Context) -> String {
        self.get_machine_name(ctx).unwrap_or_default()
    }

    fn get_machine_name(&self, ctx: &Context) -> Option<String> {
        let op = self.get_operation().deref(ctx);
        if let Ok(key) = Identifier::try_from("machine") {
            if let Some(s) = op.attributes.get::<StringAttr>(&key) {
                return Some(clean_symbol_ref(s.as_ref()).to_string());
            }
            if let Some(id) = op.attributes.get::<IdentifierAttr>(&key) {
                return Some(clean_symbol_ref(id.as_ref().as_ref()).to_string());
            }
        }
        None
    }

    /// Inputs driven to hardware instance.
    pub fn inputs(&self, ctx: &Context) -> Vec<Value> {
        let op = self.get_operation().deref(ctx);
        let num_inputs = op.get_num_operands().saturating_sub(2);
        (0..num_inputs).map(|i| op.get_operand(i)).collect()
    }

    /// Clock operand.
    pub fn clock(&self, ctx: &Context) -> Value {
        let op = self.get_operation().deref(ctx);
        let num_opds = op.get_num_operands();
        op.get_operand(num_opds - 2)
    }

    /// Reset operand.
    pub fn reset(&self, ctx: &Context) -> Value {
        let op = self.get_operation().deref(ctx);
        let num_opds = op.get_num_operands();
        op.get_operand(num_opds - 1)
    }

    /// Outputs produced by hardware instance.
    pub fn outputs(&self, ctx: &Context) -> Vec<Value> {
        let op = self.get_operation().deref(ctx);
        (0..op.get_num_results())
            .map(|i| op.get_result(i))
            .collect()
    }
}

/// Register all `fsm` operations in [Context].
pub fn register(ctx: &mut Context) {
    MachineOp::register(ctx);
    StateOp::register(ctx);
    OutputOp::register(ctx);
    TransitionOp::register(ctx);
    ReturnOp::register(ctx);
    VariableOp::register(ctx);
    UpdateOp::register(ctx);
    InstanceOp::register(ctx);
    TriggerOp::register(ctx);
    HWInstanceOp::register(ctx);
}

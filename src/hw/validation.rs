// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Module-level hardware legality checks that cannot be expressed by one op.

use std::collections::HashSet;

use pliron::{
    builtin::op_interfaces::SymbolOpInterface, common_traits::Verify, context::Context,
    linked_list::ContainsLinkedList, location::Located, operation::Operation, result::Result,
    r#type::Typed, value::Value, verify_err,
};

use crate::{
    hw::ops::{InstanceOp, ModuleOp},
    seq::types::{ClockType, MemoryType, ResetType},
    sv::ops::{
        AlwaysCombOp, AlwaysFfNoResetOp, AlwaysFfOp, AssignOp, InstanceOp as SvInstanceOp,
        LogicDeclOp, MemDeclOp, MemReadOp, MemWriteOp,
    },
};

/// Validate module-wide invariants after local operation verification.
///
/// This pass checks identity and cross-operation relationships that individual
/// operation verifiers cannot see: emitted target uniqueness, instance naming,
/// clock/reset assignment boundaries, and same-address memory write conflicts.
/// It intentionally does not resolve symbols across a module collection; that
/// requires a symbol-table pass over all top-level modules.
pub fn validate_module(ctx: &Context, module: &ModuleOp) -> Result<()> {
    module.verify(ctx)?;

    let body = module.get_body(ctx);
    let mut target_names = HashSet::new();
    let mut instance_names = HashSet::new();
    let mut memory_writes: Vec<(Value, Value)> = Vec::new();
    let clock_ty = ClockType::get(ctx).into();
    let reset_ty = ResetType::get(ctx).into();

    for op_ptr in body.deref(ctx).iter(ctx) {
        let op = op_ptr.deref(ctx);

        if let Some(assign) = Operation::get_op::<AssignOp>(op_ptr, ctx) {
            let target = assign.target(ctx);
            if !target_names.insert(target.as_ref().to_owned()) {
                return verify_err!(
                    op.loc(),
                    "duplicate SV target name '{}' in module",
                    target.as_ref()
                );
            }
            let result_type = op.get_result(0).get_type(ctx);
            if result_type == clock_ty || result_type == reset_ty {
                return verify_err!(
                    op.loc(),
                    "sv.assign cannot emit a clock or reset value as ordinary data"
                );
            }
        }

        if let Some(comb) = Operation::get_op::<AlwaysCombOp>(op_ptr, ctx) {
            let target = comb
                .get_attr_comb_target(ctx)
                .expect("sv.always_comb requires comb_target");
            if !target_names.insert(target.as_ref().to_owned()) {
                return verify_err!(
                    op.loc(),
                    "duplicate SV target name '{}' in module",
                    target.as_ref()
                );
            }
        }

        if let Some(declaration) = Operation::get_op::<LogicDeclOp>(op_ptr, ctx) {
            let target = declaration
                .get_attr_logic_target(ctx)
                .expect("sv.logic_decl requires logic_target");
            if !target_names.insert(target.as_ref().to_owned()) {
                return verify_err!(
                    op.loc(),
                    "duplicate SV target name '{}' in module",
                    target.as_ref()
                );
            }
        }

        if let Some(memory) = Operation::get_op::<MemDeclOp>(op_ptr, ctx) {
            let target = memory
                .get_attr_memory_target(ctx)
                .expect("sv.mem_decl requires memory_target");
            if !target_names.insert(target.as_ref().to_owned()) {
                return verify_err!(
                    op.loc(),
                    "duplicate SV target name '{}' in module",
                    target.as_ref()
                );
            }
        }

        if let Some(read) = Operation::get_op::<MemReadOp>(op_ptr, ctx) {
            let target = read
                .get_attr_memory_read_target(ctx)
                .expect("sv.mem_read requires memory_read_target");
            if !target_names.insert(target.as_ref().to_owned()) {
                return verify_err!(
                    op.loc(),
                    "duplicate SV target name '{}' in module",
                    target.as_ref()
                );
            }
        }

        if let Some(write) = Operation::get_op::<MemWriteOp>(op_ptr, ctx) {
            let target = write
                .get_attr_memory_write_target(ctx)
                .expect("sv.mem_write requires memory_write_target");
            if !target_names.contains(target.as_ref()) {
                target_names.insert(target.as_ref().to_owned());
            }
        }

        if let Some(always) = Operation::get_op::<AlwaysFfOp>(op_ptr, ctx) {
            let target = always
                .get_attr_ff_target(ctx)
                .expect("sv.always_ff requires ff_target");
            if !target_names.insert(target.as_ref().to_owned()) {
                return verify_err!(
                    op.loc(),
                    "duplicate SV target name '{}' in module",
                    target.as_ref()
                );
            }
        }

        if let Some(always) = Operation::get_op::<AlwaysFfNoResetOp>(op_ptr, ctx) {
            let target = always
                .get_attr_ff_nr_target(ctx)
                .expect("sv.always_ff_no_reset requires ff_nr_target");
            if !target_names.insert(target.as_ref().to_owned()) {
                return verify_err!(
                    op.loc(),
                    "duplicate SV target name '{}' in module",
                    target.as_ref()
                );
            }
        }

        if let Some(instance) = Operation::get_op::<InstanceOp>(op_ptr, ctx) {
            let instance_name = instance.instance_name(ctx);
            if instance_name.as_ref().is_empty() {
                return verify_err!(op.loc(), "hw.instance name must not be empty");
            }
            if !instance_names.insert(instance_name.as_ref().to_owned()) {
                return verify_err!(
                    op.loc(),
                    "duplicate hw.instance name '{}' in module",
                    instance_name.as_ref()
                );
            }
            if instance.module_name(ctx).as_ref().as_ref().is_empty() {
                return verify_err!(op.loc(), "hw.instance module reference must not be empty");
            }
        }

        if let Some(instance) = Operation::get_op::<SvInstanceOp>(op_ptr, ctx) {
            let instance_name = instance
                .get_attr_sv_instance_name(ctx)
                .expect("sv.instance requires instance name");
            if !instance_names.insert(instance_name.as_ref().to_owned()) {
                return verify_err!(
                    op.loc(),
                    "duplicate instance name '{}' in module",
                    instance_name.as_ref()
                );
            }
        }

        if Operation::is_op::<crate::seq::ops::HLMemWriteOp>(op_ptr, ctx) {
            let memory_type = op.get_operand(1).get_type(ctx);
            if memory_type
                .deref(ctx)
                .downcast_ref::<MemoryType>()
                .is_some()
            {
                let pair = (op.get_operand(1), op.get_operand(2));
                if memory_writes.iter().any(|existing| *existing == pair) {
                    return verify_err!(
                        op.loc(),
                        "conflicting writes to the same memory and address in one module"
                    );
                }
                memory_writes.push(pair);
            }
        }
    }

    Ok(())
}

/// Validate a collection of modules and resolve every local instance target.
///
/// This is intentionally separate from `validate_module`: a single module
/// cannot prove that an instance's referenced symbol exists elsewhere.
pub fn validate_modules(ctx: &Context, modules: &[ModuleOp]) -> Result<()> {
    let symbols: HashSet<_> = modules
        .iter()
        .map(|module| module.get_symbol_name(ctx))
        .collect();
    for module in modules {
        validate_module(ctx, module)?;
        for op_ptr in module.get_body(ctx).deref(ctx).iter(ctx) {
            if let Some(instance) = Operation::get_op::<InstanceOp>(op_ptr, ctx) {
                if !symbols.contains(&instance.module_name(ctx).into()) {
                    return verify_err!(
                        op_ptr.deref(ctx).loc(),
                        "unresolved hw.instance module reference"
                    );
                }
            }
            if let Some(instance) = Operation::get_op::<SvInstanceOp>(op_ptr, ctx) {
                let name = instance
                    .get_attr_sv_instance_module(ctx)
                    .expect("sv.instance requires module name");
                if !symbols
                    .iter()
                    .any(|symbol| symbol.as_ref() == name.as_ref())
                {
                    return verify_err!(
                        op_ptr.deref(ctx).loc(),
                        "unresolved sv.instance module reference"
                    );
                }
            }
        }
    }
    Ok(())
}

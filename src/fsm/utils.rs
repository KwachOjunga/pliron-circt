use crate::fsm::ops::MachineOp;
use pliron::{
    builtin::{
        attributes::{IdentifierAttr, StringAttr},
        op_interfaces::ATTR_KEY_SYM_NAME,
        type_interfaces::FunctionTypeInterface,
        types::FunctionType,
    },
    context::Context,
    identifier::Identifier,
    linked_list::ContainsLinkedList,
    operation::Operation,
    r#type::TypeHandle,
};

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

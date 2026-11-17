use crate::plan::*;
use crate::utils::int_attr;
use pliron::{
    builtin::types::{IntegerType, Signedness},
    context::Context,
    identifier::Identifier,
    op::Op,
    r#type::TypeHandle,
    value::Value,
};
use pliron_circt::{
    hw::{ModuleOp, OutputOp},
    register_all,
    seq::{ClockType, MemoryType, ResetType},
    sv::{
        AlwaysCombOp, AlwaysFfNoResetOp, AlwaysFfOp, AssignOp, BinaryExprOp, ConcatExprOp,
        ConstantExprOp, IndexExprOp, InstanceOp, LogicDeclOp, MemDeclOp, MuxExprOp, RegDeclOp,
        SliceExprOp, UnaryExprOp, WireDeclOp,
    },
};
use rustc_hash::FxHashMap;

/// Convert a `TypeDesc` into a verified `pliron` `TypeHandle`.
pub fn desc_to_pliron_type(ctx: &mut Context, ty: &TypeDesc) -> TypeHandle {
    match ty {
        TypeDesc::Int(w) => IntegerType::get(ctx, *w, Signedness::Signless).into(),
        TypeDesc::Clock => ClockType::get(ctx).into(),
        TypeDesc::Reset => ResetType::get(ctx).into(),
        TypeDesc::Mem { elem_width, depth } => {
            let elem_ty = IntegerType::get(ctx, *elem_width, Signedness::Signless).into();
            MemoryType::get(ctx, *depth, elem_ty).into()
        }
    }
}

/// Output of Pass 7: emitted Pliron `ModuleOp` operations.
pub struct Pass7Output {
    pub modules: Vec<ModuleOp>,
}

/// Pass 7: Deterministic serial emission into a single `pliron::Context`.
///
/// Translates pre-elaborated `ModulePlan`s into hardware dialect operations:
/// `hw.module`, `sv.*` processes and declarations, and `hw.output`.
pub fn run_pass7(ctx: &mut Context, mut plans: Vec<ModulePlan>) -> Pass7Output {
    register_all(ctx);

    // Sort plans deterministically by (file_id, span.offset, name)
    plans.sort_by(|a, b| {
        a.file_id
            .cmp(&b.file_id)
            .then(a.span.offset.cmp(&b.span.offset))
            .then(a.name.cmp(&b.name))
    });

    let mut modules = Vec::with_capacity(plans.len());

    for plan in &plans {
        let mod_op = emit_single_module(ctx, plan);
        modules.push(mod_op);
    }

    Pass7Output { modules }
}

fn emit_single_module(ctx: &mut Context, plan: &ModulePlan) -> ModuleOp {
    // 1. Module input types
    let mut input_types: Vec<TypeHandle> = Vec::new();
    for p in plan.ports.iter().filter(|p| p.dir == PortDir::Input || p.dir == PortDir::Inout) {
        input_types.push(desc_to_pliron_type(ctx, &p.ty));
    }

    let mod_ident: Identifier = plan
        .name
        .as_str()
        .try_into()
        .unwrap_or_else(|_| "unknown_module".try_into().unwrap());
    let module = ModuleOp::new(ctx, mod_ident, input_types);
    let body = module.get_body(ctx);

    let mut scope: FxHashMap<String, Value> = FxHashMap::default();

    // 2. Bind input arguments
    let mut in_idx = 0;
    for p in plan.ports.iter().filter(|p| p.dir == PortDir::Input || p.dir == PortDir::Inout) {
        let arg_val = module.get_input(ctx, in_idx);
        if let Ok(id) = p.name.as_str().try_into() {
            arg_val.set_name(ctx, Some(id));
        }
        scope.insert(p.name.clone(), arg_val);
        in_idx += 1;
    }

    // 3. Declarations (wire, logic, reg, mem)
    for decl in &plan.decls {
        let pliron_ty = desc_to_pliron_type(ctx, &decl.ty);
        let res = match decl.kind {
            DeclKind::Wire => {
                let op = WireDeclOp::new(ctx, decl.name.as_str(), pliron_ty);
                let r = op.result(ctx);
                op.get_operation().insert_at_back(body, ctx);
                r
            }
            DeclKind::Logic => {
                let op = LogicDeclOp::new(ctx, decl.name.as_str(), pliron_ty);
                let r = op.result(ctx);
                op.get_operation().insert_at_back(body, ctx);
                r
            }
            DeclKind::Reg => {
                let op = RegDeclOp::new(ctx, decl.name.as_str(), pliron_ty);
                let r = op.result(ctx);
                op.get_operation().insert_at_back(body, ctx);
                r
            }
            DeclKind::Mem => {
                let op = MemDeclOp::new(ctx, decl.name.as_str(), pliron_ty);
                let r = op.result(ctx);
                op.get_operation().insert_at_back(body, ctx);
                r
            }
        };

        if let Ok(id) = decl.name.as_str().try_into() {
            res.set_name(ctx, Some(id));
        }
        scope.insert(decl.name.clone(), res);
    }

    // Helper to emit an expression from plan.exprs
    fn emit_expr(
        ctx: &mut Context,
        expr_id: ExprId,
        plan: &ModulePlan,
        scope: &FxHashMap<String, Value>,
        body: pliron::context::Ptr<pliron::basic_block::BasicBlock>,
        expr_cache: &mut FxHashMap<ExprId, Value>,
    ) -> Value {
        if let Some(val) = expr_cache.get(&expr_id) {
            return *val;
        }

        let desc = &plan.exprs[expr_id.0 as usize];
        let res = match desc {
            ExprDesc::Constant { val, width, .. } => {
                let val_attr = int_attr(ctx, *width, *val);
                let w_attr = int_attr(ctx, 32, *width as u64);
                let ty: TypeHandle = IntegerType::get(ctx, *width, Signedness::Signless).into();
                let op = ConstantExprOp::new(ctx, val_attr, w_attr, ty);
                let r = op.result(ctx);
                op.get_operation().insert_at_back(body, ctx);
                r
            }
            ExprDesc::Ident { name, .. } => {
                if let Some(val) = scope.get(name) {
                    *val
                } else {
                    // Fallback constant 0 if unresolved
                    let val_attr = int_attr(ctx, 32, 0);
                    let w_attr = int_attr(ctx, 32, 32);
                    let ty: TypeHandle = IntegerType::get(ctx, 32, Signedness::Signless).into();
                    let op = ConstantExprOp::new(ctx, val_attr, w_attr, ty);
                    let r = op.result(ctx);
                    op.get_operation().insert_at_back(body, ctx);
                    r
                }
            }
            ExprDesc::Binary {
                op,
                lhs,
                rhs,
                res_ty,
                ..
            } => {
                let lhs_val = emit_expr(ctx, *lhs, plan, scope, body, expr_cache);
                let rhs_val = emit_expr(ctx, *rhs, plan, scope, body, expr_cache);
                let ty = desc_to_pliron_type(ctx, res_ty);
                let bin_op = BinaryExprOp::new(ctx, op.as_str(), lhs_val, rhs_val, ty);
                let r = bin_op.result(ctx);
                bin_op.get_operation().insert_at_back(body, ctx);
                r
            }
            ExprDesc::Unary {
                op,
                operand,
                res_ty,
                ..
            } => {
                let op_val = emit_expr(ctx, *operand, plan, scope, body, expr_cache);
                let ty = desc_to_pliron_type(ctx, res_ty);
                let un_op = UnaryExprOp::new(ctx, op.as_str(), op_val, ty);
                let r = un_op.result(ctx);
                un_op.get_operation().insert_at_back(body, ctx);
                r
            }
            ExprDesc::Mux {
                cond,
                true_val,
                false_val,
                res_ty,
                ..
            } => {
                let cond_v = emit_expr(ctx, *cond, plan, scope, body, expr_cache);
                let true_v = emit_expr(ctx, *true_val, plan, scope, body, expr_cache);
                let false_v = emit_expr(ctx, *false_val, plan, scope, body, expr_cache);
                let ty = desc_to_pliron_type(ctx, res_ty);
                let mux_op = MuxExprOp::new(ctx, cond_v, true_v, false_v, ty);
                let r = mux_op.result(ctx);
                mux_op.get_operation().insert_at_back(body, ctx);
                r
            }
            ExprDesc::Concat { parts, res_ty, .. } => {
                let mut vals = Vec::with_capacity(parts.len());
                for p in parts {
                    vals.push(emit_expr(ctx, *p, plan, scope, body, expr_cache));
                }
                let ty = desc_to_pliron_type(ctx, res_ty);
                let concat_op = ConcatExprOp::new(ctx, vals, ty);
                let r = concat_op.result(ctx);
                concat_op.get_operation().insert_at_back(body, ctx);
                r
            }
            ExprDesc::Slice {
                base,
                low,
                width,
                ..
            } => {
                let base_v = emit_expr(ctx, *base, plan, scope, body, expr_cache);
                let low_attr = int_attr(ctx, 32, *low as u64);
                let w_attr = int_attr(ctx, 32, *width as u64);
                let ty: TypeHandle = IntegerType::get(ctx, *width, Signedness::Signless).into();
                let slice_op = SliceExprOp::new(ctx, base_v, low_attr, w_attr, ty);
                let r = slice_op.result(ctx);
                slice_op.get_operation().insert_at_back(body, ctx);
                r
            }
            ExprDesc::Index { base, index, .. } => {
                let base_v = emit_expr(ctx, *base, plan, scope, body, expr_cache);
                let idx_v = emit_expr(ctx, *index, plan, scope, body, expr_cache);
                let ty: TypeHandle = IntegerType::get(ctx, 1, Signedness::Signless).into();
                let idx_op = IndexExprOp::new(ctx, base_v, idx_v, ty);
                let r = idx_op.result(ctx);
                idx_op.get_operation().insert_at_back(body, ctx);
                r
            }
        };

        expr_cache.insert(expr_id, res);
        res
    }

    let mut expr_cache = FxHashMap::default();

    // 4. Continuous assignments (`sv.assign`)
    for assign in &plan.assigns {
        let rhs_val = emit_expr(ctx, assign.expr, plan, &scope, body, &mut expr_cache);
        let assign_op = AssignOp::new(ctx, assign.target.as_str(), rhs_val);
        let res = assign_op.result(ctx);
        if let Ok(id) = assign.target.as_str().try_into() {
            res.set_name(ctx, Some(id));
        }
        assign_op.get_operation().insert_at_back(body, ctx);
        scope.insert(assign.target.clone(), res);
    }

    // 5. Combinational blocks (`sv.always_comb`)
    for comb in &plan.comb_blocks {
        let val = emit_expr(ctx, comb.expr, plan, &scope, body, &mut expr_cache);
        let comb_op = AlwaysCombOp::new(ctx, comb.target.as_str(), val);
        comb_op.get_operation().insert_at_back(body, ctx);
    }

    // 6. Sequential clock processes (`sv.always_ff` and `sv.always_ff_no_reset`)
    for ff in &plan.ff_blocks {
        let clk_val = scope
            .get(&ff.clk_name)
            .copied()
            .unwrap_or_else(|| emit_expr(ctx, ff.next_val, plan, &scope, body, &mut expr_cache));
        let next_val = emit_expr(ctx, ff.next_val, plan, &scope, body, &mut expr_cache);

        if let Some(ref rst) = ff.reset {
            let rst_val = scope
                .get(&rst.rst_name)
                .copied()
                .unwrap_or_else(|| emit_expr(ctx, rst.rst_val, plan, &scope, body, &mut expr_cache));
            let rst_val_expr = emit_expr(ctx, rst.rst_val, plan, &scope, body, &mut expr_cache);
            let ff_op = AlwaysFfOp::new(
                ctx,
                ff.target.as_str(),
                clk_val,
                next_val,
                rst_val,
                rst_val_expr,
                rst.is_async,
                &*rst.polarity,
            );
            ff_op.get_operation().insert_at_back(body, ctx);
        } else {
            let ff_op = AlwaysFfNoResetOp::new(ctx, ff.target.as_str(), clk_val, next_val);
            ff_op.get_operation().insert_at_back(body, ctx);
        }
    }

    // 7. Module instantiations (`sv.instance`)
    for inst in &plan.instances {
        let mut port_vals = Vec::with_capacity(inst.port_connections.len());
        for (_, expr_id) in &inst.port_connections {
            port_vals.push(emit_expr(ctx, *expr_id, plan, &scope, body, &mut expr_cache));
        }
        let inst_op = InstanceOp::new(ctx, inst.inst_name.as_str(), inst.target_mod.as_str(), port_vals);
        inst_op.get_operation().insert_at_back(body, ctx);
    }

    // 8. Outputs (`hw.output`)
    let mut output_vals: Vec<Value> = Vec::new();
    for p in plan.ports.iter().filter(|p| p.dir == PortDir::Output) {
        if let Some(val) = scope.get(&p.name) {
            output_vals.push(*val);
        } else {
            // Undriven output fallback
            let w = p.ty.bit_width();
            let z_val = int_attr(ctx, w, 0);
            let w_val = int_attr(ctx, 32, w as u64);
            let ty: TypeHandle = IntegerType::get(ctx, w, Signedness::Signless).into();
            let z_op = ConstantExprOp::new(ctx, z_val, w_val, ty);
            let res = z_op.result(ctx);
            z_op.get_operation().insert_at_back(body, ctx);
            output_vals.push(res);
        }
    }

    let out_op = OutputOp::new(ctx, output_vals);
    out_op.get_operation().insert_at_back(body, ctx);

    module
}

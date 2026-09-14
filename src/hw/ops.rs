// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Operations defined in the `hw` dialect.

use pliron::{
    attribute::AttributeDict,
    basic_block::BasicBlock,
    builtin::{
        attributes::{IdentifierAttr, IntegerAttr, StringAttr},
        op_interfaces::{
            self, IsTerminatorInterface, IsolatedFromAboveInterface, NOpdsInterface,
            NRegionsInterface, NResultsInterface, OneRegionInterface, OneResultInterface,
            RegionKind, RegionKindInterface, SingleBlockRegionInterface, SymbolOpInterface,
        },
        types::IntegerType,
    },
    combine::{Parser, optional, token},
    common_traits::Verify,
    context::{Context, Ptr},
    derive::{op_interface_impl, pliron_op},
    identifier::Identifier,
    input_err,
    irfmt::{
        parsers::spaced,
        printers::op::{region, symb_op_header},
    },
    linked_list::ContainsLinkedList,
    location::{Located, Location},
    op::{Op, OpObj},
    operation::Operation,
    parsable::{Parsable, ParseResult, StateStream},
    printable::{self, Printable},
    result::Result,
    r#type::{TypeHandle, Typed},
    value::Value,
    verify_err,
};

use super::types::{ArrayType, EnumType, InoutType, IntType, StructType, TypeAliasType, UnionType};

/// Compute the total bitwidth of a hardware type, if statically known.
pub fn compute_type_bitwidth(ctx: &Context, ty: TypeHandle) -> Option<u64> {
    let ty_ref = ty.deref(ctx);
    if let Some(int_ty) = ty_ref.downcast_ref::<IntegerType>() {
        return Some(int_ty.width() as u64);
    }
    if let Some(int_ty) = ty_ref.downcast_ref::<IntType>() {
        return Some(int_ty.width() as u64);
    }
    if let Some(arr_ty) = ty_ref.downcast_ref::<ArrayType>() {
        let elem_w = compute_type_bitwidth(ctx, arr_ty.element_type())?;
        return Some(arr_ty.size().checked_mul(elem_w)?);
    }
    if let Some(st_ty) = ty_ref.downcast_ref::<StructType>() {
        let mut total = 0u64;
        for field in st_ty.fields() {
            total = total.checked_add(compute_type_bitwidth(ctx, field.ty)?)?;
        }
        return Some(total);
    }
    if let Some(u_ty) = ty_ref.downcast_ref::<UnionType>() {
        let mut max_w = 0u64;
        for field in u_ty.fields() {
            let w = compute_type_bitwidth(ctx, field.ty)?;
            if w > max_w {
                max_w = w;
            }
        }
        return Some(max_w);
    }
    if let Some(alias_ty) = ty_ref.downcast_ref::<TypeAliasType>() {
        return compute_type_bitwidth(ctx, alias_ty.inner_type());
    }
    if let Some(enum_ty) = ty_ref.downcast_ref::<EnumType>() {
        return compute_type_bitwidth(ctx, enum_ty.underlying_type());
    }
    if let Some(inout_ty) = ty_ref.downcast_ref::<InoutType>() {
        return compute_type_bitwidth(ctx, inout_ty.element_type());
    }
    None
}

/// Hardware module container operation.
///
/// An `hw.module` contains a single [RegionKind::Graph] region with a single
/// basic block, representing concurrent hardware wires and gates.
#[pliron_op(
    name = "hw.module",
    interfaces = [
        OneRegionInterface,
        SingleBlockRegionInterface,
        SymbolOpInterface,
        IsolatedFromAboveInterface,
        NOpdsInterface<0>,
        NResultsInterface<0>,
    ],
)]
pub struct ModuleOp;

impl Verify for ModuleOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let region = self.get_region(ctx);
        let block_ptr = match region.deref(ctx).get_entry_block() {
            Some(b) => b,
            None => {
                return verify_err!(
                    op.loc(),
                    "hw.module body region must contain at least one block"
                );
            }
        };
        let block = block_ptr.deref(ctx);
        if block.get_head().is_some() && block.get_terminator(ctx).is_none() {
            return verify_err!(op.loc(), "hw.module block must end with a terminator");
        }
        Ok(())
    }
}

#[op_interface_impl]
impl RegionKindInterface for ModuleOp {
    #[inline(always)]
    fn get_region_kind(&self, _idx: usize) -> RegionKind {
        RegionKind::Graph
    }

    #[inline(always)]
    fn has_ssa_dominance(&self, _idx: usize) -> bool {
        false
    }
}

impl ModuleOp {
    /// Create a new `hw.module` with the specified symbol name and input port types.
    ///
    /// The entry block arguments correspond to the module's input ports.
    pub fn new(ctx: &mut Context, name: Identifier, input_types: Vec<TypeHandle>) -> Self {
        let op = Operation::new(ctx, Self::get_concrete_op_info(), vec![], vec![], vec![], 1);
        let module = ModuleOp { op };
        module.set_symbol_name(ctx, name);

        // Create the single block with block arguments for inputs.
        let region = module.get_region(ctx);
        let block = BasicBlock::new(ctx, None, input_types);
        block.insert_at_front(region, ctx);

        module
    }

    /// Get the body basic block of this module.
    pub fn get_body(&self, ctx: &Context) -> Ptr<BasicBlock> {
        self.get_region(ctx)
            .deref(ctx)
            .get_entry_block()
            .expect("hw.module has a body block")
    }

    /// Get an input port as an SSA value by index.
    pub fn get_input(&self, ctx: &Context, idx: usize) -> Value {
        self.get_body(ctx).deref(ctx).get_argument(idx)
    }

    /// Get the total number of input ports.
    pub fn num_inputs(&self, ctx: &Context) -> usize {
        self.get_body(ctx).deref(ctx).get_num_arguments()
    }
}

impl Printable for ModuleOp {
    fn fmt(
        &self,
        ctx: &Context,
        state: &printable::State,
        f: &mut core::fmt::Formatter<'_>,
    ) -> core::fmt::Result {
        symb_op_header(self).fmt(ctx, state, f)?;
        write!(f, " ")?;
        region(self).fmt(ctx, state, f)?;
        Ok(())
    }
}

impl Parsable for ModuleOp {
    type Arg = Vec<(Identifier, Location)>;
    type Parsed = OpObj;
    fn parse<'a>(
        state_stream: &mut StateStream<'a>,
        results: Self::Arg,
    ) -> ParseResult<'a, Self::Parsed> {
        if !results.is_empty() {
            input_err!(
                state_stream.loc(),
                op_interfaces::NResultsVerifyErr(0, results.len())
            )?
        }
        let op = Operation::new(
            state_stream.state.ctx,
            Self::get_concrete_op_info(),
            vec![],
            vec![],
            vec![],
            0,
        );
        let mut parser = (
            spaced(token('@').with(Identifier::parser(()))),
            spaced(optional(AttributeDict::parser(()))),
            spaced(pliron::region::Region::parser(op)),
        );
        parser
            .parse_stream(state_stream)
            .map(|(name, attributes, _region)| -> OpObj {
                let ctx = &mut state_stream.state.ctx;
                op.deref_mut(ctx).attributes = attributes.unwrap_or_default();
                let opop = ModuleOp { op };
                opop.set_symbol_name(ctx, name);
                OpObj::new(opop)
            })
            .into()
    }
}

/// Module output terminator operation.
///
/// Connects internal SSA signals to the enclosing module's outputs.
#[pliron_op(
    name = "hw.output",
    format,
    interfaces = [IsTerminatorInterface, NResultsInterface<0>],
)]
pub struct OutputOp;

impl Verify for OutputOp {
    fn verify(&self, _ctx: &Context) -> Result<()> {
        Ok(())
    }
}

impl OutputOp {
    /// Create a new `hw.output` operation driving the given values.
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

    /// Get the number of output ports driven by this terminator.
    pub fn get_num_outputs(&self, ctx: &Context) -> usize {
        self.get_operation().deref(ctx).get_num_operands()
    }

    /// Get the driven output value by index.
    pub fn get_output(&self, ctx: &Context, idx: usize) -> Value {
        self.get_operation().deref(ctx).get_operand(idx)
    }
}

/// Constant bitvector materialization operation.
#[pliron_op(
    name = "hw.constant",
    format = "attr($value, $IntegerAttr) ` : ` type($0)",
    interfaces = [NOpdsInterface<0>, OneResultInterface],
    attributes = (value: IntegerAttr),
)]
pub struct ConstantOp;

impl Verify for ConstantOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let val_attr = match self.get_attr_value(ctx) {
            Some(attr) => attr,
            None => return verify_err!(op.loc(), "hw.constant requires value attribute"),
        };
        let res_ty = op.get_result(0).get_type(ctx);
        if let Some(int_ty) = res_ty.deref(ctx).downcast_ref::<IntegerType>() {
            let attr_w = val_attr.get_type().deref(ctx).width();
            if attr_w != int_ty.width() {
                return verify_err!(
                    op.loc(),
                    "hw.constant attribute width ({}) does not match result type width ({})",
                    attr_w,
                    int_ty.width()
                );
            }
        }
        Ok(())
    }
}

impl ConstantOp {
    /// Create a new `hw.constant` producing a constant value.
    pub fn new(ctx: &mut Context, value_attr: IntegerAttr) -> Self {
        let ty = value_attr.get_type();
        assert!(
            ty.deref(ctx).width() > 0,
            "hw.constant width must be non-zero"
        );
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![ty.into()],
            vec![],
            vec![],
            0,
        );
        let const_op = ConstantOp { op };
        const_op.set_attr_value(ctx, value_attr);
        const_op
    }

    /// Get the SSA result produced by this constant.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Submodule instance operation.
#[pliron_op(
    name = "hw.instance",
    format,
    interfaces = [NRegionsInterface<0>],
    attributes = (instance_name: StringAttr, module_name: IdentifierAttr),
)]
pub struct InstanceOp;

impl Verify for InstanceOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let inst_name = match self.get_attr_instance_name(ctx) {
            Some(n) => n,
            None => return verify_err!(op.loc(), "hw.instance requires instance_name attribute"),
        };
        if inst_name.as_ref().is_empty() {
            return verify_err!(op.loc(), "hw.instance instance_name cannot be empty");
        }
        let _mod_name = match self.get_attr_module_name(ctx) {
            Some(m) => m,
            None => return verify_err!(op.loc(), "hw.instance requires module_name attribute"),
        };
        Ok(())
    }
}

impl InstanceOp {
    /// Create a new `hw.instance` instantiating `module_name` as `instance_name`.
    pub fn new(
        ctx: &mut Context,
        instance_name: StringAttr,
        module_name: IdentifierAttr,
        inputs: Vec<Value>,
        output_types: Vec<TypeHandle>,
    ) -> Self {
        assert!(
            !instance_name.as_ref().is_empty(),
            "hw.instance instance_name cannot be empty"
        );
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            output_types,
            inputs,
            vec![],
            0,
        );
        let instance = InstanceOp { op };
        instance.set_attr_instance_name(ctx, instance_name);
        instance.set_attr_module_name(ctx, module_name);
        instance
    }

    /// Get instance name attribute.
    pub fn instance_name(&self, ctx: &Context) -> StringAttr {
        self.get_attr_instance_name(ctx).unwrap().clone()
    }

    /// Get instantiated module name attribute.
    pub fn module_name(&self, ctx: &Context) -> IdentifierAttr {
        self.get_attr_module_name(ctx).unwrap().clone()
    }

    /// Get output results of the instance.
    pub fn results(&self, ctx: &Context) -> Vec<Value> {
        let op = self.get_operation().deref(ctx);
        (0..op.get_num_results())
            .map(|i| op.get_result(i))
            .collect()
    }
}

/// External hardware module declaration operation (ASIC macro, PLL, standard cell, IP blackbox).
#[pliron_op(
    name = "hw.module_extern",
    format,
    interfaces = [
        NRegionsInterface<0>,
        SymbolOpInterface,
        IsolatedFromAboveInterface,
        NOpdsInterface<0>,
        NResultsInterface<0>,
    ],
)]
pub struct ExternModuleOp;

impl Verify for ExternModuleOp {
    fn verify(&self, _ctx: &Context) -> Result<()> {
        Ok(())
    }
}

impl ExternModuleOp {
    /// Create an external module declaration with symbol name.
    pub fn new(ctx: &mut Context, name: Identifier) -> Self {
        let op = Operation::new(ctx, Self::get_concrete_op_info(), vec![], vec![], vec![], 0);
        let extern_mod = ExternModuleOp { op };
        extern_mod.set_symbol_name(ctx, name);
        extern_mod
    }
}

/// Explicit hardware wire declaration operation.
///
/// Gives hardware identity and debug/physical net name to an SSA signal,
/// adhering to AGENTS.md §13 (Connectivity and Drivers).
#[pliron_op(
    name = "hw.wire",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<1>],
    attributes = (name: StringAttr),
)]
pub struct WireOp;

impl Verify for WireOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let name = match self.get_attr_name(ctx) {
            Some(n) => n,
            None => return verify_err!(op.loc(), "hw.wire requires name attribute"),
        };
        if name.as_ref().is_empty() {
            return verify_err!(op.loc(), "hw.wire name cannot be empty");
        }
        let in_ty = op.get_operand(0).get_type(ctx);
        let res_ty = op.get_result(0).get_type(ctx);
        if in_ty != res_ty {
            return verify_err!(
                op.loc(),
                "hw.wire input type {} does not match result type {}",
                in_ty.disp(ctx),
                res_ty.disp(ctx)
            );
        }
        Ok(())
    }
}

impl WireOp {
    /// Create a new `hw.wire` binding an input signal to a named hardware net.
    pub fn new(ctx: &mut Context, name: StringAttr, input: Value) -> Self {
        assert!(!name.as_ref().is_empty(), "hw.wire name cannot be empty");
        let ty = input.get_type(ctx);
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![ty],
            vec![input],
            vec![],
            0,
        );
        let wire = WireOp { op };
        wire.set_attr_name(ctx, name);
        wire
    }

    /// Get the net name attribute.
    pub fn name(&self, ctx: &Context) -> StringAttr {
        self.get_attr_name(ctx).unwrap().clone()
    }

    /// Get the input value driving this wire.
    pub fn input(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(0)
    }

    /// Get the output wire SSA value.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Bit-level reinterpretation cast between types of equal total bitwidth.
#[pliron_op(
    name = "hw.bitcast",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<1>],
)]
pub struct BitcastOp;

impl Verify for BitcastOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let in_ty = op.get_operand(0).get_type(ctx);
        let res_ty = op.get_result(0).get_type(ctx);
        let in_w = compute_type_bitwidth(ctx, in_ty);
        let res_w = compute_type_bitwidth(ctx, res_ty);
        if let (Some(w_in), Some(w_res)) = (in_w, res_w) {
            if w_in != w_res {
                return verify_err!(
                    op.loc(),
                    "hw.bitcast input bitwidth ({}) must equal result bitwidth ({}); use comb.zext/sext/trunc for width changes",
                    w_in,
                    w_res
                );
            }
        }
        Ok(())
    }
}

impl BitcastOp {
    /// Create a new `hw.bitcast` casting `input` to `result_type`.
    pub fn new(ctx: &mut Context, input: Value, result_type: TypeHandle) -> Self {
        let in_ty = input.get_type(ctx);
        if let (Some(w_in), Some(w_res)) = (
            compute_type_bitwidth(ctx, in_ty),
            compute_type_bitwidth(ctx, result_type),
        ) {
            assert_eq!(
                w_in, w_res,
                "hw.bitcast input bitwidth ({}) must equal result bitwidth ({}); width conversions belong in comb",
                w_in, w_res
            );
        }
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![result_type],
            vec![input],
            vec![],
            0,
        );
        BitcastOp { op }
    }

    /// Get input operand.
    pub fn input(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_operand(0)
    }

    /// Get cast result.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Bitvector concatenation operation: combines N operands into a single wider bitvector.
#[pliron_op(
    name = "hw.concat",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface],
)]
pub struct ConcatOp;

impl Verify for ConcatOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        if op.get_num_operands() == 0 {
            return verify_err!(op.loc(), "hw.concat requires at least one operand");
        }
        let mut sum_w = 0u64;
        for i in 0..op.get_num_operands() {
            let opd_ty = op.get_operand(i).get_type(ctx);
            match compute_type_bitwidth(ctx, opd_ty) {
                Some(w) => sum_w += w,
                None => {
                    return verify_err!(op.loc(), "hw.concat operand {} has unknown bitwidth", i);
                }
            }
        }
        let res_ty = op.get_result(0).get_type(ctx);
        match compute_type_bitwidth(ctx, res_ty) {
            Some(w_res) => {
                if sum_w != w_res {
                    return verify_err!(
                        op.loc(),
                        "hw.concat result bitwidth mismatch: expected sum of inputs ({}), found {}",
                        sum_w,
                        w_res
                    );
                }
            }
            None => return verify_err!(op.loc(), "hw.concat result has unknown bitwidth"),
        }
        Ok(())
    }
}

impl ConcatOp {
    /// Create a new `hw.concat` concatenating `inputs` into `result_type`.
    pub fn new(ctx: &mut Context, inputs: Vec<Value>, result_type: TypeHandle) -> Self {
        assert!(
            !inputs.is_empty(),
            "hw.concat requires at least one operand"
        );
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![result_type],
            inputs,
            vec![],
            0,
        );
        ConcatOp { op }
    }

    /// Get the concatenation result.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Bitvector slice operation: extracts contiguous bits starting at `low_bit`.
#[pliron_op(
    name = "hw.slice",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<1>],
    attributes = (low_bit: IntegerAttr),
)]
pub struct SliceOp;

impl Verify for SliceOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let low_attr = match self.get_attr_low_bit(ctx) {
            Some(a) => a,
            None => return verify_err!(op.loc(), "hw.slice requires low_bit attribute"),
        };
        let low = low_attr.value().to_u64();
        let in_ty = op.get_operand(0).get_type(ctx);
        let res_ty = op.get_result(0).get_type(ctx);
        if let (Some(w_in), Some(w_res)) = (
            compute_type_bitwidth(ctx, in_ty),
            compute_type_bitwidth(ctx, res_ty),
        ) {
            if low.checked_add(w_res).map_or(true, |sum| sum > w_in) {
                return verify_err!(
                    op.loc(),
                    "hw.slice out of bounds: low_bit ({}) + result width ({}) > input width ({})",
                    low,
                    w_res,
                    w_in
                );
            }
        }
        Ok(())
    }
}

impl SliceOp {
    /// Create a new `hw.slice` extracting a bit-slice from `input` starting at `low_bit`.
    pub fn new(
        ctx: &mut Context,
        input: Value,
        low_bit: IntegerAttr,
        result_type: TypeHandle,
    ) -> Self {
        let in_ty = input.get_type(ctx);
        let in_w = compute_type_bitwidth(ctx, in_ty);
        let res_w = compute_type_bitwidth(ctx, result_type);
        let low = low_bit.value().to_u64();
        if let (Some(w_in), Some(w_res)) = (in_w, res_w) {
            assert!(
                low.checked_add(w_res).map_or(false, |sum| sum <= w_in),
                "hw.slice low_bit ({}) + result width ({}) exceeds input width ({})",
                low,
                w_res,
                w_in
            );
        }
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![result_type],
            vec![input],
            vec![],
            0,
        );
        let slice = SliceOp { op };
        slice.set_attr_low_bit(ctx, low_bit);
        slice
    }

    /// Get the starting bit index attribute.
    pub fn low_bit(&self, ctx: &Context) -> IntegerAttr {
        self.get_attr_low_bit(ctx).unwrap().clone()
    }

    /// Get the extracted slice result.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Array creation operation: constructs an `hw.array` from an element list.
#[pliron_op(
    name = "hw.array_create",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface],
)]
pub struct ArrayCreateOp;

impl Verify for ArrayCreateOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let res_ty = op.get_result(0).get_type(ctx);
        let res_ty_ref = res_ty.deref(ctx);
        let arr_ty = match res_ty_ref.downcast_ref::<ArrayType>() {
            Some(a) => a,
            None => return verify_err!(op.loc(), "hw.array_create result must be an ArrayType"),
        };
        if op.get_num_operands() as u64 != arr_ty.size() {
            return verify_err!(
                op.loc(),
                "hw.array_create operand count ({}) != array size ({})",
                op.get_num_operands(),
                arr_ty.size()
            );
        }
        for i in 0..op.get_num_operands() {
            let opd_ty = op.get_operand(i).get_type(ctx);
            if opd_ty != arr_ty.element_type() {
                return verify_err!(op.loc(), "hw.array_create element {} type mismatch", i);
            }
        }
        Ok(())
    }
}

impl ArrayCreateOp {
    /// Create a new `hw.array_create` from elements into `array_type`.
    pub fn new(ctx: &mut Context, elements: Vec<Value>, array_type: TypeHandle) -> Self {
        assert!(
            !elements.is_empty(),
            "hw.array_create requires at least one element"
        );
        if let Some(arr_ty) = array_type.deref(ctx).downcast_ref::<ArrayType>() {
            assert_eq!(
                elements.len() as u64,
                arr_ty.size(),
                "hw.array_create element count must match array size"
            );
        }
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![array_type],
            elements,
            vec![],
            0,
        );
        ArrayCreateOp { op }
    }

    /// Get created array result.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Array get operation: indexes into an `hw.array` with a bitvector index.
#[pliron_op(
    name = "hw.array_get",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<2>],
)]
pub struct ArrayGetOp;

impl Verify for ArrayGetOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let arr_ty_handle = op.get_operand(0).get_type(ctx);
        let arr_ty_ref = arr_ty_handle.deref(ctx);
        let arr_ty = match arr_ty_ref.downcast_ref::<ArrayType>() {
            Some(a) => a,
            None => return verify_err!(op.loc(), "hw.array_get operand 0 must be an ArrayType"),
        };
        let res_ty = op.get_result(0).get_type(ctx);
        if res_ty != arr_ty.element_type() {
            return verify_err!(
                op.loc(),
                "hw.array_get result type does not match array element type"
            );
        }
        Ok(())
    }
}

impl ArrayGetOp {
    /// Create a new `hw.array_get` indexing `array` by `index`.
    pub fn new(ctx: &mut Context, array: Value, index: Value, elem_type: TypeHandle) -> Self {
        let arr_val_ty = array.get_type(ctx);
        if let Some(arr_ty) = arr_val_ty.deref(ctx).downcast_ref::<ArrayType>() {
            assert_eq!(
                arr_ty.element_type(),
                elem_type,
                "hw.array_get element type mismatch"
            );
        }
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![elem_type],
            vec![array, index],
            vec![],
            0,
        );
        ArrayGetOp { op }
    }

    /// Get indexed element result.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Array slice operation: extracts a sub-array from an `hw.array`.
#[pliron_op(
    name = "hw.array_slice",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<2>],
)]
pub struct ArraySliceOp;

impl Verify for ArraySliceOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let arr_ty_handle = op.get_operand(0).get_type(ctx);
        let arr_ty_ref = arr_ty_handle.deref(ctx);
        let arr_ty = match arr_ty_ref.downcast_ref::<ArrayType>() {
            Some(a) => a,
            None => return verify_err!(op.loc(), "hw.array_slice operand 0 must be an ArrayType"),
        };
        let res_ty = op.get_result(0).get_type(ctx);
        let res_ty_ref = res_ty.deref(ctx);
        let slice_ty = match res_ty_ref.downcast_ref::<ArrayType>() {
            Some(s) => s,
            None => return verify_err!(op.loc(), "hw.array_slice result must be an ArrayType"),
        };
        if arr_ty.element_type() != slice_ty.element_type() {
            return verify_err!(op.loc(), "hw.array_slice element type mismatch");
        }
        if slice_ty.size() > arr_ty.size() {
            return verify_err!(op.loc(), "hw.array_slice slice size exceeds array size");
        }
        Ok(())
    }
}

impl ArraySliceOp {
    /// Create a new `hw.array_slice`.
    pub fn new(ctx: &mut Context, array: Value, low_index: Value, slice_type: TypeHandle) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![slice_type],
            vec![array, low_index],
            vec![],
            0,
        );
        ArraySliceOp { op }
    }

    /// Get slice result.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Array concatenation operation: joins multiple arrays of identical element types.
#[pliron_op(
    name = "hw.array_concat",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface],
)]
pub struct ArrayConcatOp;

impl Verify for ArrayConcatOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        if op.get_num_operands() == 0 {
            return verify_err!(op.loc(), "hw.array_concat requires at least one operand");
        }
        let res_ty = op.get_result(0).get_type(ctx);
        let res_ty_ref = res_ty.deref(ctx);
        let res_arr = match res_ty_ref.downcast_ref::<ArrayType>() {
            Some(a) => a,
            None => return verify_err!(op.loc(), "hw.array_concat result must be an ArrayType"),
        };
        let mut total_size = 0u64;
        for i in 0..op.get_num_operands() {
            let opd_ty = op.get_operand(i).get_type(ctx);
            let opd_ty_ref = opd_ty.deref(ctx);
            let opd_arr = match opd_ty_ref.downcast_ref::<ArrayType>() {
                Some(a) => a,
                None => {
                    return verify_err!(
                        op.loc(),
                        "hw.array_concat operand {} must be an ArrayType",
                        i
                    );
                }
            };
            if opd_arr.element_type() != res_arr.element_type() {
                return verify_err!(
                    op.loc(),
                    "hw.array_concat operand {} element type mismatch",
                    i
                );
            }
            total_size += opd_arr.size();
        }
        if total_size != res_arr.size() {
            return verify_err!(
                op.loc(),
                "hw.array_concat size mismatch: sum ({}) != result ({})",
                total_size,
                res_arr.size()
            );
        }
        Ok(())
    }
}

impl ArrayConcatOp {
    /// Create a new `hw.array_concat`.
    pub fn new(ctx: &mut Context, arrays: Vec<Value>, result_type: TypeHandle) -> Self {
        assert!(
            !arrays.is_empty(),
            "hw.array_concat requires at least one operand"
        );
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![result_type],
            arrays,
            vec![],
            0,
        );
        ArrayConcatOp { op }
    }

    /// Get concatenated array result.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Array element injection operation: functionally updates an element at `index`.
#[pliron_op(
    name = "hw.array_inject",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<3>],
)]
pub struct ArrayInjectOp;

impl Verify for ArrayInjectOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let arr_ty_handle = op.get_operand(0).get_type(ctx);
        let arr_ty_ref = arr_ty_handle.deref(ctx);
        let arr_ty = match arr_ty_ref.downcast_ref::<ArrayType>() {
            Some(a) => a,
            None => return verify_err!(op.loc(), "hw.array_inject operand 0 must be an ArrayType"),
        };
        let new_val_ty = op.get_operand(2).get_type(ctx);
        if new_val_ty != arr_ty.element_type() {
            return verify_err!(
                op.loc(),
                "hw.array_inject new_val type does not match array element type"
            );
        }
        let res_ty = op.get_result(0).get_type(ctx);
        if res_ty != arr_ty_handle {
            return verify_err!(
                op.loc(),
                "hw.array_inject result type must match input array type"
            );
        }
        Ok(())
    }
}

impl ArrayInjectOp {
    /// Create a new `hw.array_inject`.
    pub fn new(
        ctx: &mut Context,
        array: Value,
        index: Value,
        new_val: Value,
        array_type: TypeHandle,
    ) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![array_type],
            vec![array, index, new_val],
            vec![],
            0,
        );
        ArrayInjectOp { op }
    }

    /// Get updated array result.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Struct creation operation: packs fields into an `hw.struct`.
#[pliron_op(
    name = "hw.struct_create",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface],
)]
pub struct StructCreateOp;

impl Verify for StructCreateOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let res_ty = op.get_result(0).get_type(ctx);
        let res_ty_ref = res_ty.deref(ctx);
        let st_ty = match res_ty_ref.downcast_ref::<StructType>() {
            Some(s) => s,
            None => return verify_err!(op.loc(), "hw.struct_create result must be a StructType"),
        };
        if op.get_num_operands() != st_ty.fields().len() {
            return verify_err!(
                op.loc(),
                "hw.struct_create operand count ({}) does not match struct field count ({})",
                op.get_num_operands(),
                st_ty.fields().len()
            );
        }
        for (i, field) in st_ty.fields().iter().enumerate() {
            let opd_ty = op.get_operand(i).get_type(ctx);
            if opd_ty != field.ty {
                return verify_err!(
                    op.loc(),
                    "hw.struct_create field '{}' type mismatch",
                    field.name
                );
            }
        }
        Ok(())
    }
}

impl StructCreateOp {
    /// Create a new `hw.struct_create` packing `fields` into `struct_type`.
    pub fn new(ctx: &mut Context, fields: Vec<Value>, struct_type: TypeHandle) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![struct_type],
            fields,
            vec![],
            0,
        );
        StructCreateOp { op }
    }

    /// Get struct result.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Struct field extraction operation: reads a named field from an `hw.struct`.
#[pliron_op(
    name = "hw.struct_extract",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<1>],
    attributes = (field_name: StringAttr),
)]
pub struct StructExtractOp;

impl Verify for StructExtractOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let f_name = match self.get_attr_field_name(ctx) {
            Some(f) => f,
            None => {
                return verify_err!(op.loc(), "hw.struct_extract requires field_name attribute");
            }
        };
        if f_name.as_ref().is_empty() {
            return verify_err!(op.loc(), "hw.struct_extract field_name cannot be empty");
        }
        let st_val_ty = op.get_operand(0).get_type(ctx);
        let st_val_ty_ref = st_val_ty.deref(ctx);
        let st_ty = match st_val_ty_ref.downcast_ref::<StructType>() {
            Some(s) => s,
            None => return verify_err!(op.loc(), "hw.struct_extract operand must be a StructType"),
        };
        let field = match st_ty
            .fields()
            .iter()
            .find(|f| f.name.to_string() == *f_name.as_ref())
        {
            Some(f) => f,
            None => {
                return verify_err!(
                    op.loc(),
                    "hw.struct_extract field '{}' not found in struct",
                    f_name.as_ref()
                );
            }
        };
        let res_ty = op.get_result(0).get_type(ctx);
        if res_ty != field.ty {
            return verify_err!(
                op.loc(),
                "hw.struct_extract result type does not match field type"
            );
        }
        Ok(())
    }
}

impl StructExtractOp {
    /// Create a new `hw.struct_extract`.
    pub fn new(
        ctx: &mut Context,
        struct_val: Value,
        field_name: StringAttr,
        field_type: TypeHandle,
    ) -> Self {
        assert!(
            !field_name.as_ref().is_empty(),
            "hw.struct_extract field_name cannot be empty"
        );
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![field_type],
            vec![struct_val],
            vec![],
            0,
        );
        let extract = StructExtractOp { op };
        extract.set_attr_field_name(ctx, field_name);
        extract
    }

    /// Get extracted field result.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Struct field injection operation: functionally updates a named field.
#[pliron_op(
    name = "hw.struct_inject",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<2>],
    attributes = (target_field: StringAttr),
)]
pub struct StructInjectOp;

impl Verify for StructInjectOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let f_name = match self.get_attr_target_field(ctx) {
            Some(f) => f,
            None => {
                return verify_err!(op.loc(), "hw.struct_inject requires target_field attribute");
            }
        };
        if f_name.as_ref().is_empty() {
            return verify_err!(op.loc(), "hw.struct_inject target_field cannot be empty");
        }
        let st_val_ty = op.get_operand(0).get_type(ctx);
        let st_val_ty_ref = st_val_ty.deref(ctx);
        let st_ty = match st_val_ty_ref.downcast_ref::<StructType>() {
            Some(s) => s,
            None => {
                return verify_err!(op.loc(), "hw.struct_inject operand 0 must be a StructType");
            }
        };
        let field = match st_ty
            .fields()
            .iter()
            .find(|f| f.name.to_string() == *f_name.as_ref())
        {
            Some(f) => f,
            None => {
                return verify_err!(
                    op.loc(),
                    "hw.struct_inject field '{}' not found in struct",
                    f_name.as_ref()
                );
            }
        };
        let new_val_ty = op.get_operand(1).get_type(ctx);
        if new_val_ty != field.ty {
            return verify_err!(
                op.loc(),
                "hw.struct_inject new_val type does not match field type"
            );
        }
        let res_ty = op.get_result(0).get_type(ctx);
        if res_ty != st_val_ty {
            return verify_err!(
                op.loc(),
                "hw.struct_inject result type must match input struct type"
            );
        }
        Ok(())
    }
}

impl StructInjectOp {
    /// Create a new `hw.struct_inject`.
    pub fn new(
        ctx: &mut Context,
        struct_val: Value,
        field_name: StringAttr,
        new_val: Value,
        struct_type: TypeHandle,
    ) -> Self {
        assert!(
            !field_name.as_ref().is_empty(),
            "hw.struct_inject target_field cannot be empty"
        );
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![struct_type],
            vec![struct_val, new_val],
            vec![],
            0,
        );
        let inject = StructInjectOp { op };
        inject.set_attr_target_field(ctx, field_name);
        inject
    }

    /// Get updated struct result.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Struct explode operation: unpacks all fields into individual SSA results.
#[pliron_op(
    name = "hw.struct_explode",
    format,
    interfaces = [NRegionsInterface<0>, NOpdsInterface<1>],
)]
pub struct StructExplodeOp;

impl Verify for StructExplodeOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let st_val_ty = op.get_operand(0).get_type(ctx);
        let st_val_ty_ref = st_val_ty.deref(ctx);
        let st_ty = match st_val_ty_ref.downcast_ref::<StructType>() {
            Some(s) => s,
            None => return verify_err!(op.loc(), "hw.struct_explode operand must be a StructType"),
        };
        if op.get_num_results() != st_ty.fields().len() {
            return verify_err!(
                op.loc(),
                "hw.struct_explode result count ({}) != struct fields ({})",
                op.get_num_results(),
                st_ty.fields().len()
            );
        }
        for (i, field) in st_ty.fields().iter().enumerate() {
            let res_ty = op.get_result(i).get_type(ctx);
            if res_ty != field.ty {
                return verify_err!(
                    op.loc(),
                    "hw.struct_explode result {} type mismatch for field '{}'",
                    i,
                    field.name
                );
            }
        }
        Ok(())
    }
}

impl StructExplodeOp {
    /// Create a new `hw.struct_explode`.
    pub fn new(ctx: &mut Context, struct_val: Value, field_types: Vec<TypeHandle>) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            field_types,
            vec![struct_val],
            vec![],
            0,
        );
        StructExplodeOp { op }
    }

    /// Get exploded field results.
    pub fn results(&self, ctx: &Context) -> Vec<Value> {
        let op = self.get_operation().deref(ctx);
        (0..op.get_num_results())
            .map(|i| op.get_result(i))
            .collect()
    }
}

/// Type declaration symbol operation.
#[pliron_op(
    name = "hw.typedecl",
    format,
    interfaces = [
        NRegionsInterface<0>,
        SymbolOpInterface,
        NOpdsInterface<0>,
        NResultsInterface<0>,
    ],
)]
pub struct TypeDeclOp;

impl Verify for TypeDeclOp {
    fn verify(&self, _ctx: &Context) -> Result<()> {
        Ok(())
    }
}

impl TypeDeclOp {
    /// Create a new `hw.typedecl` with symbol name.
    pub fn new(ctx: &mut Context, name: Identifier) -> Self {
        let op = Operation::new(ctx, Self::get_concrete_op_info(), vec![], vec![], vec![], 0);
        let typedecl = TypeDeclOp { op };
        typedecl.set_symbol_name(ctx, name);
        typedecl
    }
}

/// Union creation operation: creates an `hw.union` tagged variant value.
#[pliron_op(
    name = "hw.union_create",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<1>],
    attributes = (union_tag: StringAttr),
)]
pub struct UnionCreateOp;

impl Verify for UnionCreateOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let tag = match self.get_attr_union_tag(ctx) {
            Some(t) => t,
            None => return verify_err!(op.loc(), "hw.union_create requires union_tag attribute"),
        };
        if tag.as_ref().is_empty() {
            return verify_err!(op.loc(), "hw.union_create union_tag cannot be empty");
        }
        let res_ty = op.get_result(0).get_type(ctx);
        let res_ty_ref = res_ty.deref(ctx);
        let u_ty = match res_ty_ref.downcast_ref::<UnionType>() {
            Some(u) => u,
            None => return verify_err!(op.loc(), "hw.union_create result must be a UnionType"),
        };
        let field = match u_ty
            .fields()
            .iter()
            .find(|f| f.name.to_string() == *tag.as_ref())
        {
            Some(f) => f,
            None => {
                return verify_err!(
                    op.loc(),
                    "hw.union_create field '{}' not found in union",
                    tag.as_ref()
                );
            }
        };
        let val_ty = op.get_operand(0).get_type(ctx);
        if val_ty != field.ty {
            return verify_err!(
                op.loc(),
                "hw.union_create operand type does not match field '{}' type",
                tag.as_ref()
            );
        }
        Ok(())
    }
}

impl UnionCreateOp {
    /// Create a new `hw.union_create`.
    pub fn new(
        ctx: &mut Context,
        val: Value,
        field_name: StringAttr,
        union_type: TypeHandle,
    ) -> Self {
        assert!(
            !field_name.as_ref().is_empty(),
            "hw.union_create field_name cannot be empty"
        );
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![union_type],
            vec![val],
            vec![],
            0,
        );
        let create = UnionCreateOp { op };
        create.set_attr_union_tag(ctx, field_name);
        create
    }

    /// Get union result.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Union field extraction operation: reads a variant from an `hw.union`.
#[pliron_op(
    name = "hw.union_extract",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<1>],
    attributes = (extract_tag: StringAttr),
)]
pub struct UnionExtractOp;

impl Verify for UnionExtractOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let tag = match self.get_attr_extract_tag(ctx) {
            Some(t) => t,
            None => {
                return verify_err!(op.loc(), "hw.union_extract requires extract_tag attribute");
            }
        };
        if tag.as_ref().is_empty() {
            return verify_err!(op.loc(), "hw.union_extract extract_tag cannot be empty");
        }
        let u_val_ty = op.get_operand(0).get_type(ctx);
        let u_val_ty_ref = u_val_ty.deref(ctx);
        let u_ty = match u_val_ty_ref.downcast_ref::<UnionType>() {
            Some(u) => u,
            None => return verify_err!(op.loc(), "hw.union_extract operand must be a UnionType"),
        };
        let field = match u_ty
            .fields()
            .iter()
            .find(|f| f.name.to_string() == *tag.as_ref())
        {
            Some(f) => f,
            None => {
                return verify_err!(
                    op.loc(),
                    "hw.union_extract field '{}' not found in union",
                    tag.as_ref()
                );
            }
        };
        let res_ty = op.get_result(0).get_type(ctx);
        if res_ty != field.ty {
            return verify_err!(
                op.loc(),
                "hw.union_extract result type does not match field '{}' type",
                tag.as_ref()
            );
        }
        Ok(())
    }
}

impl UnionExtractOp {
    /// Create a new `hw.union_extract`.
    pub fn new(
        ctx: &mut Context,
        union_val: Value,
        field_name: StringAttr,
        field_type: TypeHandle,
    ) -> Self {
        assert!(
            !field_name.as_ref().is_empty(),
            "hw.union_extract field_name cannot be empty"
        );
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![field_type],
            vec![union_val],
            vec![],
            0,
        );
        let extract = UnionExtractOp { op };
        extract.set_attr_extract_tag(ctx, field_name);
        extract
    }

    /// Get extracted field result.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Hardware module parameter declaration symbol operation: `hw.param_decl`.
#[pliron_op(
    name = "hw.param_decl",
    format,
    interfaces = [
        NRegionsInterface<0>,
        SymbolOpInterface,
        NOpdsInterface<0>,
        NResultsInterface<0>,
    ],
    attributes = (param_type: StringAttr, default_val: StringAttr),
)]
pub struct ParamDeclOp;

impl Verify for ParamDeclOp {
    fn verify(&self, _ctx: &Context) -> Result<()> {
        Ok(())
    }
}

impl ParamDeclOp {
    /// Create a new `hw.param_decl`.
    pub fn new(
        ctx: &mut Context,
        name: Identifier,
        param_type: StringAttr,
        default_val: StringAttr,
    ) -> Self {
        let op = Operation::new(ctx, Self::get_concrete_op_info(), vec![], vec![], vec![], 0);
        let decl = ParamDeclOp { op };
        decl.set_symbol_name(ctx, name);
        decl.set_attr_param_type(ctx, param_type);
        decl.set_attr_default_val(ctx, default_val);
        decl
    }
}

/// Hardware parameter value reference operation: `hw.param_value`.
#[pliron_op(
    name = "hw.param_value",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface, NOpdsInterface<0>],
    attributes = (param_ref: StringAttr),
)]
pub struct ParamValueOp;

impl Verify for ParamValueOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let pref = match self.get_attr_param_ref(ctx) {
            Some(p) => p,
            None => return verify_err!(op.loc(), "hw.param_value requires param_ref attribute"),
        };
        if pref.as_ref().is_empty() {
            return verify_err!(op.loc(), "hw.param_value param_ref cannot be empty");
        }
        Ok(())
    }
}

impl ParamValueOp {
    /// Create a new `hw.param_value`.
    pub fn new(ctx: &mut Context, param_ref: StringAttr, result_type: TypeHandle) -> Self {
        assert!(
            !param_ref.as_ref().is_empty(),
            "hw.param_value param_ref cannot be empty"
        );
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![result_type],
            vec![],
            vec![],
            0,
        );
        let val_op = ParamValueOp { op };
        val_op.set_attr_param_ref(ctx, param_ref);
        val_op
    }

    /// Get parameter value result.
    pub fn result(&self, ctx: &Context) -> Value {
        self.get_operation().deref(ctx).get_result(0)
    }
}

/// Hardware hierarchical path symbol declaration: `hw.hierpath`.
///
/// Encodes a sequence of instance/module references designating an element deep
/// within the module instance hierarchy.
#[pliron_op(
    name = "hw.hierpath",
    format,
    interfaces = [
        NRegionsInterface<0>,
        SymbolOpInterface,
        NOpdsInterface<0>,
        NResultsInterface<0>,
    ],
    attributes = (path_string: StringAttr),
)]
pub struct HierPathOp;

impl Verify for HierPathOp {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let op = self.get_operation().deref(ctx);
        let p_str = match self.get_attr_path_string(ctx) {
            Some(p) => p,
            None => return verify_err!(op.loc(), "hw.hierpath requires path_string attribute"),
        };
        if p_str.as_ref().is_empty() {
            return verify_err!(op.loc(), "hw.hierpath path_string cannot be empty");
        }
        Ok(())
    }
}

impl HierPathOp {
    /// Create a new `hw.hierpath`.
    pub fn new(ctx: &mut Context, name: Identifier, path: StringAttr) -> Self {
        assert!(
            !path.as_ref().is_empty(),
            "hw.hierpath path cannot be empty"
        );
        let op = Operation::new(ctx, Self::get_concrete_op_info(), vec![], vec![], vec![], 0);
        let hp = HierPathOp { op };
        hp.set_symbol_name(ctx, name);
        hp.set_attr_path_string(ctx, path);
        hp
    }
}

/// Register all operations in the `hw` dialect.
pub fn register(ctx: &mut Context) {
    ModuleOp::register(ctx);
    ExternModuleOp::register(ctx);
    OutputOp::register(ctx);
    ConstantOp::register(ctx);
    InstanceOp::register(ctx);
    WireOp::register(ctx);
    BitcastOp::register(ctx);
    ConcatOp::register(ctx);
    SliceOp::register(ctx);
    ArrayCreateOp::register(ctx);
    ArrayGetOp::register(ctx);
    ArraySliceOp::register(ctx);
    ArrayConcatOp::register(ctx);
    ArrayInjectOp::register(ctx);
    StructCreateOp::register(ctx);
    StructExtractOp::register(ctx);
    StructInjectOp::register(ctx);
    StructExplodeOp::register(ctx);
    TypeDeclOp::register(ctx);
    UnionCreateOp::register(ctx);
    UnionExtractOp::register(ctx);
    ParamDeclOp::register(ctx);
    ParamValueOp::register(ctx);
    HierPathOp::register(ctx);
}

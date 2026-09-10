// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Operations defined in the `hw` dialect.

use pliron::{
    attribute::AttributeDict,
    basic_block::BasicBlock,
    builtin::{
        attributes::{IdentifierAttr, IntegerAttr, StringAttr},
        op_interfaces::{
            self, ATTR_KEY_SYM_NAME, IsTerminatorInterface, IsolatedFromAboveInterface,
            NOpdsInterface, NRegionsInterface, NResultsInterface, OneRegionInterface,
            OneResultInterface, RegionKind, RegionKindInterface, SingleBlockRegionInterface,
            SymbolOpInterface,
        },
    },
    combine::{Parser, optional, token},
    context::{Context, Ptr},
    derive::{op_interface_impl, pliron_op},
    identifier::Identifier,
    input_err,
    irfmt::{
        parsers::spaced,
        printers::op::{region, symb_op_header},
    },
    location::Location,
    op::{Op, OpObj},
    operation::Operation,
    parsable::{IntoParseResult, Parsable, ParseResult, StateStream},
    printable::{self, Printable},
    r#type::TypeHandle,
    value::Value,
};

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
    verifier = "succ",
)]
pub struct ModuleOp;

#[op_interface_impl]
impl RegionKindInterface for ModuleOp {
    fn get_region_kind(&self, _idx: usize) -> RegionKind {
        RegionKind::Graph
    }

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
    verifier = "succ",
)]
pub struct OutputOp;

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
    pub fn num_outputs(&self, ctx: &Context) -> usize {
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
    verifier = "succ",
)]
pub struct ConstantOp;

impl ConstantOp {
    /// Create a new `hw.constant` producing a constant value.
    pub fn new(ctx: &mut Context, value_attr: IntegerAttr) -> Self {
        let ty = value_attr.get_type();
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
    verifier = "succ",
)]
pub struct InstanceOp;

impl InstanceOp {
    /// Create a new `hw.instance` instantiating `module_name` as `instance_name`.
    pub fn new(
        ctx: &mut Context,
        instance_name: StringAttr,
        module_name: IdentifierAttr,
        inputs: Vec<Value>,
        output_types: Vec<TypeHandle>,
    ) -> Self {
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
        self.get_attr_instance_name(ctx)
    }

    /// Get instantiated module name attribute.
    pub fn module_name(&self, ctx: &Context) -> IdentifierAttr {
        self.get_attr_module_name(ctx)
    }

    /// Get output results of the instance.
    pub fn results(&self, ctx: &Context) -> Vec<Value> {
        let op = self.get_operation().deref(ctx);
        (0..op.get_num_results()).map(|i| op.get_result(i)).collect()
    }
}

/// External hardware module declaration operation (ASIC macro, PLL, standard cell, IP blackbox).
#[pliron_op(
    name = "hw.module.extern",
    format,
    interfaces = [
        NRegionsInterface<0>,
        SymbolOpInterface,
        IsolatedFromAboveInterface,
        NOpdsInterface<0>,
        NResultsInterface<0>,
    ],
    verifier = "succ",
)]
pub struct ExternModuleOp;

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
    verifier = "succ",
)]
pub struct WireOp;

impl WireOp {
    /// Create a new `hw.wire` binding an input signal to a named hardware net.
    pub fn new(ctx: &mut Context, name: StringAttr, input: Value) -> Self {
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
        self.get_attr_name(ctx)
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
    verifier = "succ",
)]
pub struct BitcastOp;

impl BitcastOp {
    /// Create a new `hw.bitcast` casting `input` to `result_type`.
    pub fn new(ctx: &mut Context, input: Value, result_type: TypeHandle) -> Self {
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
    verifier = "succ",
)]
pub struct ConcatOp;

impl ConcatOp {
    /// Create a new `hw.concat` concatenating `inputs` into `result_type`.
    pub fn new(ctx: &mut Context, inputs: Vec<Value>, result_type: TypeHandle) -> Self {
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
    verifier = "succ",
)]
pub struct SliceOp;

impl SliceOp {
    /// Create a new `hw.slice` extracting a bit-slice from `input` starting at `low_bit`.
    pub fn new(
        ctx: &mut Context,
        input: Value,
        low_bit: IntegerAttr,
        result_type: TypeHandle,
    ) -> Self {
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
        self.get_attr_low_bit(ctx)
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
    verifier = "succ",
)]
pub struct ArrayCreateOp;

impl ArrayCreateOp {
    /// Create a new `hw.array_create` from elements into `array_type`.
    pub fn new(ctx: &mut Context, elements: Vec<Value>, array_type: TypeHandle) -> Self {
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
    verifier = "succ",
)]
pub struct ArrayGetOp;

impl ArrayGetOp {
    /// Create a new `hw.array_get` indexing `array` by `index`.
    pub fn new(ctx: &mut Context, array: Value, index: Value, elem_type: TypeHandle) -> Self {
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
    verifier = "succ",
)]
pub struct ArraySliceOp;

impl ArraySliceOp {
    /// Create a new `hw.array_slice`.
    pub fn new(
        ctx: &mut Context,
        array: Value,
        low_index: Value,
        slice_type: TypeHandle,
    ) -> Self {
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
    verifier = "succ",
)]
pub struct ArrayConcatOp;

impl ArrayConcatOp {
    /// Create a new `hw.array_concat`.
    pub fn new(ctx: &mut Context, arrays: Vec<Value>, result_type: TypeHandle) -> Self {
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

/// Struct creation operation: packs fields into an `hw.struct`.
#[pliron_op(
    name = "hw.struct_create",
    format,
    interfaces = [NRegionsInterface<0>, OneResultInterface],
    verifier = "succ",
)]
pub struct StructCreateOp;

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
    verifier = "succ",
)]
pub struct StructExtractOp;

impl StructExtractOp {
    /// Create a new `hw.struct_extract`.
    pub fn new(
        ctx: &mut Context,
        struct_val: Value,
        field_name: StringAttr,
        field_type: TypeHandle,
    ) -> Self {
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
    attributes = (field_name: StringAttr),
    verifier = "succ",
)]
pub struct StructInjectOp;

impl StructInjectOp {
    /// Create a new `hw.struct_inject`.
    pub fn new(
        ctx: &mut Context,
        struct_val: Value,
        field_name: StringAttr,
        new_val: Value,
        struct_type: TypeHandle,
    ) -> Self {
        let op = Operation::new(
            ctx,
            Self::get_concrete_op_info(),
            vec![struct_type],
            vec![struct_val, new_val],
            vec![],
            0,
        );
        let inject = StructInjectOp { op };
        inject.set_attr_field_name(ctx, field_name);
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
    verifier = "succ",
)]
pub struct StructExplodeOp;

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
        (0..op.get_num_results()).map(|i| op.get_result(i)).collect()
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
    verifier = "succ",
)]
pub struct TypeDeclOp;

impl TypeDeclOp {
    /// Create a new `hw.typedecl` with symbol name.
    pub fn new(ctx: &mut Context, name: Identifier) -> Self {
        let op = Operation::new(ctx, Self::get_concrete_op_info(), vec![], vec![], vec![], 0);
        let typedecl = TypeDeclOp { op };
        typedecl.set_symbol_name(ctx, name);
        typedecl
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
    StructCreateOp::register(ctx);
    StructExtractOp::register(ctx);
    StructInjectOp::register(ctx);
    StructExplodeOp::register(ctx);
    TypeDeclOp::register(ctx);
}

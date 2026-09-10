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
}

/// Register all operations in the `hw` dialect.
pub fn register(ctx: &mut Context) {
    ModuleOp::register(ctx);
    OutputOp::register(ctx);
    ConstantOp::register(ctx);
    InstanceOp::register(ctx);
}

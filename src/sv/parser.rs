// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! SystemVerilog parser targeting the `sv`, `hw`, and `seq` dialects.
//!
//! Parses synthesizable SystemVerilog module definitions into verified Pliron hardware IR
//! using the IEEE 1800-2017 compliant `sv-parser` library.

use awint::bw;
use pliron::{
    builtin::{
        attributes::IntegerAttr,
        types::{IntegerType, Signedness},
    },
    context::{Context, Ptr},
    identifier::Identifier,
    location::Location,
    op::Op,
    result::Result,
    r#type::{TypeHandle, Typed},
    utils::apint::APInt,
    value::Value,
    verify_err, verify_error,
};
use rustc_hash::FxHashMap;
use std::{
    string::{String, ToString},
    vec::Vec,
};

use crate::{
    hw::ops::{ModuleOp, OutputOp},
    seq::types::{ClockType, MemoryType, ResetType},
    sv::ops::{
        AlwaysCombOp, AlwaysFfNoResetOp, AlwaysFfOp, AssignOp, BinaryExprOp, ConcatExprOp,
        ConstantExprOp, IndexExprOp, InstanceOp, LogicDeclOp, MemDeclOp, MuxExprOp, RegDeclOp,
        SliceExprOp, UnaryExprOp, WireDeclOp,
    },
};
use sv_parser::{RefNode, SyntaxTree, parse_sv_str, unwrap_node};

/// Port definition parsed from a module header.
#[derive(Debug, Clone)]
pub struct ParsedPort {
    pub name: String,
    pub is_input: bool,
    pub is_output: bool,
    pub bit_width: u32,
    pub is_clock: bool,
    pub is_reset: bool,
}

/// Helper to construct integer attributes with specified bit width.
pub fn int_attr(ctx: &mut Context, width: u32, val: u64) -> IntegerAttr {
    let ty = IntegerType::get(ctx, width, Signedness::Signless);
    IntegerAttr::new(ty, APInt::from_u64(val, bw(width as usize)))
}

/// Parses numeric literals (sized base and unsized decimal) into `(value, width)`.
pub fn parse_number_literal(s: &str) -> (u64, u32) {
    if let Some((width_part, val_part)) = s.split_once('\'') {
        let width: u32 = width_part.parse().unwrap_or(32);
        let val_str =
            val_part.trim_start_matches(['b', 'B', 'd', 'D', 'h', 'H', 's', 'S', 'o', 'O']);
        let radix = if val_part.starts_with(['h', 'H']) || val_part.contains(['h', 'H']) {
            16
        } else if val_part.starts_with(['b', 'B']) || val_part.contains(['b', 'B']) {
            2
        } else if val_part.starts_with(['o', 'O']) || val_part.contains(['o', 'O']) {
            8
        } else {
            10
        };
        let value = u64::from_str_radix(&val_str.replace('_', ""), radix).unwrap_or(0);
        (value, width)
    } else {
        let value: u64 = s.replace('_', "").parse().unwrap_or(0);
        (value, 32)
    }
}

/// Lowers an `sv-parser` Concrete Syntax Tree (`SyntaxTree`) into Pliron hardware IR.
pub struct SvModuleLowerer<'a> {
    ctx: &'a mut Context,
    source: &'a str,
    tree: &'a SyntaxTree,
    scope: FxHashMap<String, Value>,
    ports: Vec<ParsedPort>,
}

impl<'a> SvModuleLowerer<'a> {
    pub fn new(ctx: &'a mut Context, source: &'a str, tree: &'a SyntaxTree) -> Self {
        Self {
            ctx,
            source,
            tree,
            scope: FxHashMap::default(),
            ports: Vec::new(),
        }
    }

    /// Retrieve the trimmed source text for any CST node or collection of nodes.
    fn node_text(&self, node: impl IntoIterator<Item = RefNode<'a>>) -> &'a str {
        let mut beg = None;
        let mut end = 0;
        for n in node {
            if let RefNode::Locate(x) = n {
                if beg.is_none() {
                    beg = Some(x.offset);
                }
                end = x.offset + x.len;
            }
        }
        if let Some(beg) = beg {
            self.source[beg..end].trim()
        } else {
            ""
        }
    }

    /// Convert a CST locate into a Pliron location.
    fn locate_to_loc(&self, _node: impl IntoIterator<Item = RefNode<'a>>) -> Location {
        Location::Unknown
    }

    /// Entry point to lower a SystemVerilog source tree into a `ModuleOp`.
    pub fn lower_module(&mut self) -> Result<ModuleOp> {
        let module_ansi = unwrap_node!(self.tree, ModuleDeclarationAnsi);
        let module_nonansi = unwrap_node!(self.tree, ModuleDeclarationNonansi);

        match (module_ansi, module_nonansi) {
            (Some(RefNode::ModuleDeclarationAnsi(m)), _) => self.lower_module_ansi(m),
            (_, Some(RefNode::ModuleDeclarationNonansi(m))) => self.lower_module_nonansi(m),
            _ => verify_err!(
                Location::Unknown,
                "no supported SystemVerilog module declaration found in syntax tree"
            ),
        }
    }

    /// Lower an ANSI style module declaration: `module foo (...); ... endmodule`.
    fn lower_module_ansi(&mut self, m: &'a sv_parser::ModuleDeclarationAnsi) -> Result<ModuleOp> {
        let header = &m.nodes.0;
        let mod_id_node = &header.nodes.3;
        let mod_name = self.node_text(mod_id_node).to_string();
        if mod_name.is_empty() {
            return verify_err!(Location::Unknown, "module name cannot be empty");
        }

        // Parse ports from header
        if let Some(ref port_list) = header.nodes.6 {
            self.ports = self.lower_ansi_port_list(port_list)?;
        }

        // Construct input types
        let mut input_types: Vec<TypeHandle> = Vec::new();
        for p in self.ports.iter().filter(|p| p.is_input) {
            let ty: TypeHandle = if p.is_clock {
                ClockType::get(self.ctx).into()
            } else if p.is_reset {
                ResetType::get(self.ctx).into()
            } else {
                IntegerType::get(self.ctx, p.bit_width, Signedness::Signless).into()
            };
            input_types.push(ty);
        }

        let module_ident: Identifier = mod_name.as_str().try_into().map_err(|_| {
            verify_error!(
                Location::Unknown,
                "invalid module identifier '{}'",
                mod_name
            )
        })?;
        let module = ModuleOp::new(self.ctx, module_ident, input_types);
        let body = module.get_body(self.ctx);

        // Bind input arguments to scope
        let mut in_idx = 0;
        for p in &self.ports {
            if p.is_input {
                let arg_val = module.get_input(self.ctx, in_idx);
                if let Ok(id) = p.name.as_str().try_into() {
                    arg_val.set_name(self.ctx, Some(id));
                }
                self.scope.insert(p.name.clone(), arg_val);
                in_idx += 1;
            }
        }

        // Lower body items: Vec<NonPortModuleItem>
        for item in &m.nodes.2 {
            self.lower_non_port_module_item(item, body)?;
        }

        // Collect outputs
        self.finalize_outputs(body)?;

        Ok(module)
    }

    /// Stub for non-ANSI style module declaration.
    fn lower_module_nonansi(
        &mut self,
        m: &'a sv_parser::ModuleDeclarationNonansi,
    ) -> Result<ModuleOp> {
        let loc = self.locate_to_loc(m);
        verify_err!(loc, "non-ANSI module declarations are not yet supported")
    }

    /// Parse ANSI port list into structured `ParsedPort` vector.
    fn lower_ansi_port_list(
        &mut self,
        list_node: &'a sv_parser::ListOfPortDeclarations,
    ) -> Result<Vec<ParsedPort>> {
        let mut ports = Vec::new();
        let mut prev_dir = (true, false); // Default to input

        for node in list_node {
            if let RefNode::AnsiPortDeclaration(ansi_port) = node {
                let (name, dir, width) = self.lower_single_ansi_port(ansi_port, prev_dir)?;
                prev_dir = dir;

                let is_clock = (name == "clk" || name == "clock") && width == 1;
                let is_reset = (name == "rst" || name == "reset") && width == 1;

                ports.push(ParsedPort {
                    name,
                    is_input: dir.0,
                    is_output: dir.1,
                    bit_width: width,
                    is_clock,
                    is_reset,
                });
            }
        }

        Ok(ports)
    }

    /// Lower a single ANSI port declaration.
    fn lower_single_ansi_port(
        &self,
        port: &'a sv_parser::AnsiPortDeclaration,
        prev_dir: (bool, bool),
    ) -> Result<(String, (bool, bool), u32)> {
        let id_node = unwrap_node!(port, PortIdentifier)
            .ok_or_else(|| verify_error!(Location::Unknown, "port missing identifier"))?;
        let name = self.node_text(id_node).to_string();

        let dir = if let Some(dir_node) = unwrap_node!(port, PortDirection) {
            match dir_node {
                RefNode::PortDirection(sv_parser::PortDirection::Input(_)) => (true, false),
                RefNode::PortDirection(sv_parser::PortDirection::Output(_)) => (false, true),
                RefNode::PortDirection(sv_parser::PortDirection::Inout(_)) => (true, true),
                RefNode::PortDirection(sv_parser::PortDirection::Ref(_)) => {
                    return verify_err!(Location::Unknown, "ref ports are not supported in RTL");
                }
                _ => prev_dir,
            }
        } else {
            prev_dir
        };

        let width = self.extract_range_width(port)?;

        Ok((name, dir, width))
    }

    /// Extract bit width from a node containing optional range expressions.
    fn extract_range_width(&self, node: impl IntoIterator<Item = RefNode<'a>>) -> Result<u32> {
        if let Some(RefNode::ConstantRange(range)) = unwrap_node!(node, ConstantRange) {
            let nodes = range.nodes.clone();
            let msb_text = self.node_text(&nodes.0);
            let lsb_text = self.node_text(&nodes.2);
            let (msb, _) = parse_number_literal(msb_text);
            let (lsb, _) = parse_number_literal(lsb_text);
            let w = if msb >= lsb {
                (msb - lsb + 1) as u32
            } else {
                (lsb - msb + 1) as u32
            };
            Ok(w)
        } else {
            Ok(1)
        }
    }

    /// Lower a module body item (`NonPortModuleItem`).
    fn lower_non_port_module_item(
        &mut self,
        item: &'a sv_parser::NonPortModuleItem,
        body: Ptr<pliron::basic_block::BasicBlock>,
    ) -> Result<()> {
        // Continuous assignment: assign x = expr;
        if let Some(assign) = unwrap_node!(item, ContinuousAssign) {
            return self.lower_continuous_assign(assign, body);
        }

        // Net declaration: wire [7:0] w;
        if let Some(net_decl) = unwrap_node!(item, NetDeclaration) {
            return self.lower_net_declaration(net_decl, body);
        }

        // Data declaration: logic [7:0] l; reg [7:0] r; logic [7:0] mem [0:15];
        if let Some(data_decl) = unwrap_node!(item, DataDeclaration) {
            return self.lower_data_declaration(data_decl, body);
        }

        // Always blocks: always_comb, always_ff
        if let Some(always) = unwrap_node!(item, AlwaysConstruct) {
            return self.lower_always_construct(always, body);
        }

        // Module instantiation: child_mod u1 (.a(x), .b(y));
        if let Some(inst) = unwrap_node!(item, ModuleInstantiation) {
            return self.lower_module_instantiation(inst, body);
        }

        let loc = self.locate_to_loc(item);
        self.stub_unsupported_item("unsupported module item", loc)
    }

    /// Lower continuous assignment `assign lhs = rhs;` into `sv.assign`.
    fn lower_continuous_assign(
        &mut self,
        assign: RefNode<'a>,
        body: Ptr<pliron::basic_block::BasicBlock>,
    ) -> Result<()> {
        let (target_name, expr_node) = match assign {
            RefNode::ContinuousAssign(sv_parser::ContinuousAssign::Net(n)) => {
                let net_assignment = unwrap_node!(&**n, NetAssignment).ok_or_else(|| {
                    verify_error!(Location::Unknown, "continuous assign missing assignment")
                })?;
                if let RefNode::NetAssignment(na) = net_assignment {
                    let target = self.node_text(&na.nodes.0).to_string();
                    (target, RefNode::Expression(&na.nodes.2))
                } else {
                    return verify_err!(Location::Unknown, "invalid net assignment");
                }
            }
            RefNode::ContinuousAssign(sv_parser::ContinuousAssign::Variable(v)) => {
                let var_assignment = unwrap_node!(&**v, VariableAssignment).ok_or_else(|| {
                    verify_error!(Location::Unknown, "continuous assign missing assignment")
                })?;
                if let RefNode::VariableAssignment(va) = var_assignment {
                    let target = self.node_text(&va.nodes.0).to_string();
                    (target, RefNode::Expression(&va.nodes.2))
                } else {
                    return verify_err!(Location::Unknown, "invalid variable assignment");
                }
            }
            _ => return verify_err!(Location::Unknown, "expected continuous assignment"),
        };

        let rhs_val = self.lower_expression(expr_node, body)?;
        let assign_op = AssignOp::new(self.ctx, target_name.as_str(), rhs_val);
        let res = assign_op.result(self.ctx);
        if let Ok(id) = target_name.as_str().try_into() {
            res.set_name(self.ctx, Some(id));
        }
        assign_op.get_operation().insert_at_back(body, self.ctx);
        self.scope.insert(target_name, res);
        Ok(())
    }

    /// Lower net declarations: `wire [7:0] w;`.
    fn lower_net_declaration(
        &mut self,
        net_decl: RefNode<'a>,
        body: Ptr<pliron::basic_block::BasicBlock>,
    ) -> Result<()> {
        let width = self.extract_range_width(net_decl.clone())?;
        let id_node = unwrap_node!(net_decl, NetIdentifier, SimpleIdentifier, EscapedIdentifier)
            .ok_or_else(|| {
                verify_error!(Location::Unknown, "net declaration missing identifier")
            })?;
        let name = self.node_text(id_node).to_string();

        let ty: TypeHandle = IntegerType::get(self.ctx, width, Signedness::Signless).into();
        let decl_op = WireDeclOp::new(self.ctx, name.as_str(), ty);
        let res = decl_op.result(self.ctx);
        if let Ok(id) = name.as_str().try_into() {
            res.set_name(self.ctx, Some(id));
        }
        decl_op.get_operation().insert_at_back(body, self.ctx);
        self.scope.insert(name, res);
        Ok(())
    }

    /// Lower data declarations: `logic [7:0] l;` or `reg [7:0] r;` or memory `logic [7:0] mem [0:15];`.
    fn lower_data_declaration(
        &mut self,
        data_decl: RefNode<'a>,
        body: Ptr<pliron::basic_block::BasicBlock>,
    ) -> Result<()> {
        let width = self.extract_range_width(data_decl.clone())?;
        let decl_text = self.node_text(data_decl.clone());

        // Check for unpacked dimension (memory array): e.g. [0:15]
        if let Some(unpacked) =
            unwrap_node!(data_decl.clone(), UnpackedDimension, VariableDimension)
        {
            let depth = self.extract_range_width(unpacked)?;
            if depth > 1 {
                let id_node = unwrap_node!(
                    data_decl,
                    VariableIdentifier,
                    SimpleIdentifier,
                    EscapedIdentifier
                )
                .ok_or_else(|| {
                    verify_error!(Location::Unknown, "data declaration missing identifier")
                })?;
                let name = self.node_text(id_node).to_string();

                let elem_ty: TypeHandle =
                    IntegerType::get(self.ctx, width, Signedness::Signless).into();
                let mem_ty: TypeHandle = MemoryType::get(self.ctx, depth as u64, elem_ty).into();
                let mem_op = MemDeclOp::new(self.ctx, name.as_str(), mem_ty);
                let res = mem_op.result(self.ctx);
                if let Ok(id) = name.as_str().try_into() {
                    res.set_name(self.ctx, Some(id));
                }
                mem_op.get_operation().insert_at_back(body, self.ctx);
                self.scope.insert(name, res);
                return Ok(());
            }
        }

        let id_node = unwrap_node!(
            data_decl,
            VariableIdentifier,
            SimpleIdentifier,
            EscapedIdentifier
        )
        .ok_or_else(|| verify_error!(Location::Unknown, "data declaration missing identifier"))?;
        let name = self.node_text(id_node).to_string();

        let ty: TypeHandle = IntegerType::get(self.ctx, width, Signedness::Signless).into();

        if decl_text.starts_with("reg") {
            let decl_op = RegDeclOp::new(self.ctx, name.as_str(), ty);
            let res = decl_op.result(self.ctx);
            if let Ok(id) = name.as_str().try_into() {
                res.set_name(self.ctx, Some(id));
            }
            decl_op.get_operation().insert_at_back(body, self.ctx);
            self.scope.insert(name, res);
        } else {
            let decl_op = LogicDeclOp::new(self.ctx, name.as_str(), ty);
            let res = decl_op.result(self.ctx);
            if let Ok(id) = name.as_str().try_into() {
                res.set_name(self.ctx, Some(id));
            }
            decl_op.get_operation().insert_at_back(body, self.ctx);
            self.scope.insert(name, res);
        }

        Ok(())
    }

    /// Lower procedural blocks: `always_comb` and `always_ff`.
    fn lower_always_construct(
        &mut self,
        always: RefNode<'a>,
        body: Ptr<pliron::basic_block::BasicBlock>,
    ) -> Result<()> {
        if let RefNode::AlwaysConstruct(ref ac) = always {
            match ac.nodes.0 {
                sv_parser::AlwaysKeyword::AlwaysComb(_) => {
                    self.lower_always_comb(&ac.nodes.1, body)
                }
                sv_parser::AlwaysKeyword::AlwaysFf(_) => self.lower_always_ff(&ac.nodes.1, body),
                _ => {
                    let loc = self.locate_to_loc(always);
                    self.stub_unsupported_item("unsupported always construct type", loc)
                }
            }
        } else {
            verify_err!(Location::Unknown, "expected AlwaysConstruct")
        }
    }

    /// Lower `always_comb begin target = expr; end` into `sv.always_comb`.
    fn lower_always_comb(
        &mut self,
        stmt: &'a sv_parser::Statement,
        body: Ptr<pliron::basic_block::BasicBlock>,
    ) -> Result<()> {
        let (target_name, expr_node) = self.extract_assignment_from_statement(stmt)?;
        let expr_val = self.lower_expression(expr_node, body)?;
        let comb_op = AlwaysCombOp::new(self.ctx, target_name.as_str(), expr_val);
        comb_op.get_operation().insert_at_back(body, self.ctx);
        Ok(())
    }

    /// Lower `always_ff @(posedge clk ...)` into `sv.always_ff` or `sv.always_ff_no_reset`.
    fn lower_always_ff(
        &mut self,
        stmt: &'a sv_parser::Statement,
        body: Ptr<pliron::basic_block::BasicBlock>,
    ) -> Result<()> {
        let timing_stmt = unwrap_node!(stmt, ProceduralTimingControlStatement)
            .ok_or_else(|| verify_error!(Location::Unknown, "always_ff requires timing control"))?;

        let (clk_name, rst_info) =
            if let RefNode::ProceduralTimingControlStatement(ref pts) = timing_stmt {
                self.extract_clock_and_reset_events(&pts.nodes.0)?
            } else {
                return verify_err!(Location::Unknown, "invalid procedural timing statement");
            };

        let clk_val = self.resolve_val(&clk_name)?;

        // Check if there is an if-else conditional statement for reset
        if let Some(cond_stmt) = unwrap_node!(stmt, ConditionalStatement) {
            let (is_async, rst_name, rst_polarity) = if let Some((r_name, r_pol)) = rst_info {
                (true, r_name, r_pol)
            } else if let RefNode::ConditionalStatement(cs) = cond_stmt.clone() {
                let cond_text = self
                    .node_text(cs.nodes.2.nodes)
                    .trim_matches(['(', ')'])
                    .trim();
                if cond_text.starts_with(['!', '~']) {
                    let name = cond_text.trim_start_matches(['!', '~']).trim().to_string();
                    (false, name, "active_low".to_string())
                } else {
                    (false, cond_text.to_string(), "active_high".to_string())
                }
            } else {
                return verify_err!(Location::Unknown, "cannot extract reset condition");
            };

            let rst_val = self.resolve_val(&rst_name)?;

            let (target, reset_expr_node, next_expr_node) =
                self.extract_if_else_assignments(cond_stmt)?;
            let reset_val_expr = self.lower_expression(reset_expr_node, body)?;
            let next_val_expr = self.lower_expression(next_expr_node, body)?;

            let ff_op = AlwaysFfOp::new(
                self.ctx,
                target.as_str(),
                clk_val,
                next_val_expr,
                rst_val,
                reset_val_expr,
                is_async,
                &*rst_polarity,
            );
            ff_op.get_operation().insert_at_back(body, self.ctx);
        } else {
            let (target, expr_node) = self.extract_assignment_from_statement(stmt)?;
            let next_val = self.lower_expression(expr_node, body)?;
            let ff_op = AlwaysFfNoResetOp::new(self.ctx, target.as_str(), clk_val, next_val);
            ff_op.get_operation().insert_at_back(body, self.ctx);
        }

        Ok(())
    }

    /// Extract clock and optional reset event details from sensitivity event control.
    fn extract_clock_and_reset_events(
        &self,
        timing_ctrl: &'a sv_parser::ProceduralTimingControl,
    ) -> Result<(String, Option<(String, String)>)> {
        let text = self.node_text(timing_ctrl);
        let mut clk_name = String::new();
        let mut rst_info: Option<(String, String)> = None;

        let parts: Vec<&str> = text.split("or").collect();
        for (idx, part) in parts.iter().enumerate() {
            let trimmed = part
                .trim()
                .trim_start_matches('@')
                .trim()
                .trim_matches(['(', ')']);
            let is_negedge = trimmed.contains("negedge");
            let is_posedge = trimmed.contains("posedge");

            let id = trimmed
                .split_whitespace()
                .last()
                .unwrap_or("")
                .trim_matches([',', ';', ')']);

            if idx == 0 {
                clk_name = id.to_string();
            } else {
                let pol = if is_negedge {
                    "active_low".to_string()
                } else if is_posedge {
                    "active_high".to_string()
                } else {
                    "active_high".to_string()
                };
                rst_info = Some((id.to_string(), pol));
            }
        }

        if clk_name.is_empty() {
            return verify_err!(
                Location::Unknown,
                "cannot extract clock from sensitivity list"
            );
        }

        Ok((clk_name, rst_info))
    }

    /// Extract target identifier and expression from a procedural statement.
    fn extract_assignment_from_statement(
        &self,
        stmt: impl IntoIterator<Item = RefNode<'a>>,
    ) -> Result<(String, RefNode<'a>)> {
        match unwrap_node!(
            stmt,
            NonblockingAssignment,
            OperatorAssignment,
            VariableAssignment
        ) {
            Some(RefNode::NonblockingAssignment(a)) => {
                let target = self.node_text(&a.nodes.0).to_string();
                Ok((target, RefNode::Expression(&a.nodes.3)))
            }
            Some(RefNode::OperatorAssignment(a)) => {
                let target = self.node_text(&a.nodes.0).to_string();
                Ok((target, RefNode::Expression(&a.nodes.2)))
            }
            Some(RefNode::VariableAssignment(a)) => {
                let target = self.node_text(&a.nodes.0).to_string();
                Ok((target, RefNode::Expression(&a.nodes.2)))
            }
            _ => verify_err!(
                Location::Unknown,
                "cannot find assignment inside procedural block"
            ),
        }
    }

    /// Extract reset and next-state assignments from a conditional statement.
    fn extract_if_else_assignments(
        &self,
        cond: RefNode<'a>,
    ) -> Result<(String, RefNode<'a>, RefNode<'a>)> {
        if let RefNode::ConditionalStatement(cs) = cond {
            let (target_rst, rst_expr) = self.extract_assignment_from_statement(&cs.nodes.3)?;
            if let Some((_, ref else_stmt)) = cs.nodes.5 {
                let (target_next, next_expr) = self.extract_assignment_from_statement(else_stmt)?;
                if target_rst != target_next {
                    return verify_err!(
                        Location::Unknown,
                        "reset target '{}' does not match next-state target '{}'",
                        target_rst,
                        target_next
                    );
                }
                Ok((target_rst, rst_expr, next_expr))
            } else {
                verify_err!(
                    Location::Unknown,
                    "conditional statement missing else branch"
                )
            }
        } else {
            verify_err!(Location::Unknown, "expected ConditionalStatement")
        }
    }

    /// Lower module instantiation: `child_module u1 (in1, in2);` or `.a(in1)`.
    fn lower_module_instantiation(
        &mut self,
        inst: RefNode<'a>,
        body: Ptr<pliron::basic_block::BasicBlock>,
    ) -> Result<()> {
        if let RefNode::ModuleInstantiation(ref mi) = inst {
            let target_mod = self.node_text(&mi.nodes.0).to_string();

            for hi in mi.nodes.2.contents() {
                let inst_name = self.node_text(&hi.nodes.0).to_string();
                let mut port_vals = Vec::new();

                for conn in hi {
                    if let RefNode::OrderedPortConnection(opc) = conn {
                        if let Some(ref expr) = opc.nodes.1 {
                            let val = self.lower_expression(RefNode::Expression(expr), body)?;
                            port_vals.push(val);
                        }
                    } else if let RefNode::NamedPortConnection(npc) = conn {
                        if let sv_parser::NamedPortConnection::Identifier(id_conn) = npc {
                            if let Some(ref paren) = id_conn.nodes.3 {
                                if let Some(ref expr) = paren.nodes.1 {
                                    let val =
                                        self.lower_expression(RefNode::Expression(expr), body)?;
                                    port_vals.push(val);
                                }
                            } else {
                                let port_name = self.node_text(&id_conn.nodes.2);
                                let val = self.resolve_val(port_name)?;
                                port_vals.push(val);
                            }
                        }
                    }
                }

                let inst_op =
                    InstanceOp::new(self.ctx, inst_name.as_str(), target_mod.as_str(), port_vals);
                inst_op.get_operation().insert_at_back(body, self.ctx);
            }
            Ok(())
        } else {
            verify_err!(Location::Unknown, "expected ModuleInstantiation")
        }
    }

    /// Recursively lower an expression into a Pliron SSA `Value`.
    pub fn lower_expression(
        &mut self,
        expr: RefNode<'a>,
        body: Ptr<pliron::basic_block::BasicBlock>,
    ) -> Result<Value> {
        // Binary expression: a + b
        if let Some(bin) = unwrap_node!(expr.clone(), ExpressionBinary, ConstantExpressionBinary) {
            return self.lower_binary_expression(bin, body);
        }

        // Unary expression: ~a, !a
        if let Some(un) = unwrap_node!(expr.clone(), ExpressionUnary, ConstantExpressionUnary) {
            return self.lower_unary_expression(un, body);
        }

        // Conditional expression: sel ? a : b
        if let Some(cond) = unwrap_node!(
            expr.clone(),
            ConditionalExpression,
            ConstantExpressionTernary
        ) {
            return self.lower_conditional_expression(cond, body);
        }

        // Concatenation: {a, b}
        if let Some(concat) = unwrap_node!(expr.clone(), Concatenation, ConstantConcatenation) {
            return self.lower_concatenation(concat, body);
        }

        // Primary expression: numbers, identifiers, slices, parenthesized
        if let Some(prim) = unwrap_node!(expr.clone(), Primary, ConstantPrimary) {
            return self.lower_primary(prim, body);
        }

        // Direct number fallback
        if let Some(num) = unwrap_node!(expr.clone(), Number, IntegralNumber) {
            let num_text = self.node_text(num).to_string();
            return self.lower_number_literal(&num_text, body);
        }

        // Direct identifier fallback
        if let Some(id) = unwrap_node!(expr.clone(), SimpleIdentifier, EscapedIdentifier) {
            let id_text = self.node_text(id);
            return self.resolve_val(id_text);
        }

        let loc = self.locate_to_loc(expr);
        self.stub_unsupported_expression("unsupported expression", loc)
    }

    /// Lower binary expression: `a + b`, `a == b`, etc.
    fn lower_binary_expression(
        &mut self,
        bin: RefNode<'a>,
        body: Ptr<pliron::basic_block::BasicBlock>,
    ) -> Result<Value> {
        let (lhs_node, op_str, rhs_node) = match bin {
            RefNode::ExpressionBinary(b) => {
                let op = self.node_text(&b.nodes.1).to_string();
                (
                    RefNode::ExpressionBinaryOperand(&b.nodes.0),
                    op,
                    RefNode::ExpressionBinaryOperand(&b.nodes.3),
                )
            }
            RefNode::ConstantExpressionBinary(b) => {
                let op = self.node_text(&b.nodes.1).to_string();
                (
                    RefNode::ConstantExpression(&b.nodes.0),
                    op,
                    RefNode::ConstantExpression(&b.nodes.3),
                )
            }
            _ => return verify_err!(Location::Unknown, "invalid binary expression"),
        };

        let lhs = self.lower_expression(lhs_node, body)?;
        let rhs = self.lower_expression(rhs_node, body)?;
        let res_ty = lhs.get_type(self.ctx);

        let bin_op = BinaryExprOp::new(self.ctx, op_str.as_str(), lhs, rhs, res_ty);
        let res = bin_op.result(self.ctx);
        bin_op.get_operation().insert_at_back(body, self.ctx);
        Ok(res)
    }

    /// Lower unary expression: `~a`, `!a`.
    fn lower_unary_expression(
        &mut self,
        un: RefNode<'a>,
        body: Ptr<pliron::basic_block::BasicBlock>,
    ) -> Result<Value> {
        let (op_str, operand_node) = match un {
            RefNode::ExpressionUnary(u) => {
                let op = self.node_text(&u.nodes.0).to_string();
                (op, RefNode::Primary(&u.nodes.2))
            }
            RefNode::ConstantExpressionUnary(u) => {
                let op = self.node_text(&u.nodes.0).to_string();
                (op, RefNode::ConstantPrimary(&u.nodes.2))
            }
            _ => return verify_err!(Location::Unknown, "invalid unary expression"),
        };

        let operand = self.lower_expression(operand_node, body)?;
        let res_ty = operand.get_type(self.ctx);

        let un_op = UnaryExprOp::new(self.ctx, op_str.as_str(), operand, res_ty);
        let res = un_op.result(self.ctx);
        un_op.get_operation().insert_at_back(body, self.ctx);
        Ok(res)
    }

    /// Lower conditional expression: `sel ? a : b` into `sv.mux_expr`.
    fn lower_conditional_expression(
        &mut self,
        cond: RefNode<'a>,
        body: Ptr<pliron::basic_block::BasicBlock>,
    ) -> Result<Value> {
        let (pred_node, true_node, false_node) = match cond {
            RefNode::ConditionalExpression(c) => (
                RefNode::CondPredicate(&c.nodes.0),
                RefNode::Expression(&c.nodes.3),
                RefNode::Expression(&c.nodes.5),
            ),
            RefNode::ConstantExpressionTernary(c) => (
                RefNode::ConstantExpression(&c.nodes.0),
                RefNode::ConstantExpression(&c.nodes.3),
                RefNode::ConstantExpression(&c.nodes.5),
            ),
            _ => return verify_err!(Location::Unknown, "invalid conditional expression"),
        };

        let sel_val = self.lower_expression(pred_node, body)?;
        let true_val = self.lower_expression(true_node, body)?;
        let false_val = self.lower_expression(false_node, body)?;
        let res_ty = true_val.get_type(self.ctx);

        let mux_op = MuxExprOp::new(self.ctx, sel_val, true_val, false_val, res_ty);
        let res = mux_op.result(self.ctx);
        mux_op.get_operation().insert_at_back(body, self.ctx);
        Ok(res)
    }

    /// Lower concatenation expression: `{a, b}` into `sv.concat_expr`.
    fn lower_concatenation(
        &mut self,
        concat: RefNode<'a>,
        body: Ptr<pliron::basic_block::BasicBlock>,
    ) -> Result<Value> {
        let mut operands = Vec::new();
        let mut total_width: u32 = 0;

        let mut expr_nodes: Vec<RefNode<'a>> = Vec::new();
        match concat {
            RefNode::Concatenation(c) => {
                for expr in c.nodes.0.nodes.1.contents() {
                    expr_nodes.push(RefNode::Expression(expr));
                }
            }
            RefNode::ConstantConcatenation(c) => {
                for expr in c.nodes.0.nodes.1.contents() {
                    expr_nodes.push(RefNode::ConstantExpression(expr));
                }
            }
            _ => return verify_err!(Location::Unknown, "invalid concatenation"),
        }

        for expr_node in expr_nodes {
            let val = self.lower_expression(expr_node, body)?;
            let ty = val.get_type(self.ctx);
            if let Some(int_ty) = ty.deref(self.ctx).downcast_ref::<IntegerType>() {
                total_width += int_ty.width();
            } else {
                total_width += 1;
            }
            operands.push(val);
        }

        if operands.is_empty() {
            return verify_err!(Location::Unknown, "concatenation cannot be empty");
        }

        let res_ty: TypeHandle =
            IntegerType::get(self.ctx, total_width, Signedness::Signless).into();
        let concat_op = ConcatExprOp::new(self.ctx, operands, res_ty);
        let res = concat_op.result(self.ctx);
        concat_op.get_operation().insert_at_back(body, self.ctx);
        Ok(res)
    }

    /// Lower primary expressions (numbers, identifiers, slices, indexing).
    fn lower_primary(
        &mut self,
        primary: RefNode<'a>,
        body: Ptr<pliron::basic_block::BasicBlock>,
    ) -> Result<Value> {
        // Parenthesized expression: (expr)
        if let Some(inner) = unwrap_node!(primary.clone(), Expression, ConstantExpression) {
            return self.lower_expression(inner, body);
        }

        // Hierarchical primary (identifier with optional bit-select / slice)
        if let Some(hier) = unwrap_node!(primary.clone(), PrimaryHierarchical) {
            return self.lower_primary_hierarchical(hier, body);
        }

        // Numeric literals
        if let Some(num) = unwrap_node!(primary.clone(), Number, IntegralNumber) {
            let num_text = self.node_text(num).to_string();
            return self.lower_number_literal(&num_text, body);
        }

        // Direct identifier
        if let Some(id) = unwrap_node!(primary.clone(), SimpleIdentifier, EscapedIdentifier) {
            let id_text = self.node_text(id);
            return self.resolve_val(id_text);
        }

        let loc = self.locate_to_loc(primary);
        self.stub_unsupported_expression("unsupported primary expression", loc)
    }

    /// Lower hierarchical primary (ident, slice `a[msb:lsb]`, index `a[sel]`).
    fn lower_primary_hierarchical(
        &mut self,
        hier: RefNode<'a>,
        body: Ptr<pliron::basic_block::BasicBlock>,
    ) -> Result<Value> {
        let id_node = unwrap_node!(hier.clone(), HierarchicalIdentifier, SimpleIdentifier)
            .ok_or_else(|| verify_error!(Location::Unknown, "missing variable identifier"))?;
        let name = self.node_text(id_node);
        let base_val = self.resolve_val(name)?;

        // Check if there is a select: [msb:lsb] (slice) or [idx] (index)
        if let Some(select) = unwrap_node!(hier, Select) {
            if let Some(range) = unwrap_node!(select.clone(), ConstantRange) {
                let (msb, lsb) = match range {
                    RefNode::ConstantRange(r) => {
                        let const_range = r.clone();
                        let (m, _) = parse_number_literal(self.node_text(&const_range.nodes.0));
                        let (l, _) = parse_number_literal(self.node_text(&const_range.nodes.2));
                        (m, l)
                    }
                    _ => (0, 0),
                };

                let low_bit = lsb.min(msb) as u32;
                let slice_w = msb.max(lsb) as u32 - low_bit + 1;
                let low_attr = int_attr(self.ctx, 32, low_bit as u64);
                let w_attr = int_attr(self.ctx, 32, slice_w as u64);
                let slice_ty: TypeHandle =
                    IntegerType::get(self.ctx, slice_w, Signedness::Signless).into();

                let slice_op = SliceExprOp::new(self.ctx, base_val, low_attr, w_attr, slice_ty);
                let res = slice_op.result(self.ctx);
                slice_op.get_operation().insert_at_back(body, self.ctx);
                return Ok(res);
            }

            if let Some(bit_select) = unwrap_node!(select, BitSelect) {
                let idx_expr = unwrap_node!(bit_select, Expression).ok_or_else(|| {
                    verify_error!(Location::Unknown, "bit select missing index expression")
                })?;
                let idx_val = self.lower_expression(idx_expr, body)?;
                let res_ty: TypeHandle = IntegerType::get(self.ctx, 1, Signedness::Signless).into();
                let idx_op = IndexExprOp::new(self.ctx, base_val, idx_val, res_ty);
                let res = idx_op.result(self.ctx);
                idx_op.get_operation().insert_at_back(body, self.ctx);
                return Ok(res);
            }
        }

        Ok(base_val)
    }

    /// Lower numeric constant literals into `sv.constant_expr`.
    fn lower_number_literal(
        &mut self,
        text: &str,
        body: Ptr<pliron::basic_block::BasicBlock>,
    ) -> Result<Value> {
        let (val, width) = parse_number_literal(text);
        let val_attr = int_attr(self.ctx, width, val);
        let w_attr = int_attr(self.ctx, 32, width as u64);
        let ty: TypeHandle = IntegerType::get(self.ctx, width, Signedness::Signless).into();

        let const_op = ConstantExprOp::new(self.ctx, val_attr, w_attr, ty);
        let res = const_op.result(self.ctx);
        const_op.get_operation().insert_at_back(body, self.ctx);
        Ok(res)
    }

    /// Resolve an identifier against the current module symbol scope.
    fn resolve_val(&self, name: &str) -> Result<Value> {
        self.scope
            .get(name)
            .copied()
            .ok_or_else(|| verify_error!(Location::Unknown, "unresolved identifier '{}'", name))
    }

    /// Finalize module output ports with `hw.output`.
    fn finalize_outputs(&mut self, body: Ptr<pliron::basic_block::BasicBlock>) -> Result<()> {
        let mut output_vals: Vec<Value> = Vec::new();

        for p in self.ports.iter().filter(|p| p.is_output) {
            if let Some(val) = self.scope.get(&p.name) {
                output_vals.push(*val);
            } else {
                let z_val = int_attr(self.ctx, p.bit_width, 0);
                let w_val = int_attr(self.ctx, 32, p.bit_width as u64);
                let z_ty = IntegerType::get(self.ctx, p.bit_width, Signedness::Signless).into();
                let z_op = ConstantExprOp::new(self.ctx, z_val, w_val, z_ty);
                let res = z_op.result(self.ctx);
                z_op.get_operation().insert_at_back(body, self.ctx);
                output_vals.push(res);
            }
        }

        let out_op = OutputOp::new(self.ctx, output_vals);
        out_op.get_operation().insert_at_back(body, self.ctx);
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Stubs for upcoming / unsupported SystemVerilog constructs
    // -------------------------------------------------------------------------

    fn stub_unsupported_item(&self, item_name: &str, loc: Location) -> Result<()> {
        verify_err!(loc, "unsupported SystemVerilog construct: {}", item_name)
    }

    fn stub_unsupported_expression(&self, expr_name: &str, loc: Location) -> Result<Value> {
        verify_err!(loc, "unsupported SystemVerilog expression: {}", expr_name)
    }
}

/// Convenience entry point to parse a SystemVerilog source string into a Pliron `ModuleOp`.
pub fn parse_sv_module(ctx: &mut Context, source: &str) -> Result<ModuleOp> {
    let defines = std::collections::HashMap::new();
    let (tree, _defines) = parse_sv_str(source, "input.sv", &defines, &[] as &[&str], false, false)
        .map_err(|e| verify_error!(Location::Unknown, "sv-parser syntax error: {:?}", e))?;

    let mut lowerer = SvModuleLowerer::new(ctx, source, &tree);
    lowerer.lower_module()
}

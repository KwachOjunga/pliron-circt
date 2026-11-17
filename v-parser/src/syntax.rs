use crate::plan::*;
use crate::utils::parse_number_literal;
use rustc_hash::FxHashMap;
use std::path::{Path, PathBuf};
use sv_parser::{
    Define, Defines, /* Error as SvParserError,*/ RefNode, SyntaxTree, parse_sv_str,
    unwrap_node,
};

/// Type of top-level SystemVerilog definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefType {
    Module,
    Interface,
    Package,
    Program,
    Primitive,
}

/// Lightweight summary of a definition extracted from a source CST.
#[derive(Debug, Clone)]
pub struct DefSummary {
    pub name: String,
    pub def_type: DefType,
    pub span: SourceSpan,
    pub instantiated_modules: Vec<String>,
}

/// Lightweight summary of all definitions in a single compilation unit / file.
#[derive(Debug, Clone)]
pub struct FileSummary {
    pub file_id: FileId,
    pub defs: Vec<DefSummary>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Parse source text with `sv-parser` into a `SyntaxTree`.
pub fn parse_source(
    file_id: FileId,
    source: &str,
    path: &Path,
    defines: &std::collections::HashMap<Defines, Option<Define>>,
    incdirs: &[PathBuf],
) -> std::result::Result<
    (
        SyntaxTree,
        std::collections::HashMap<Defines, Option<Define>>,
    ),
    Diagnostic,
> {
    parse_sv_str(source, path, defines, incdirs, false, false).map_err(|err| Diagnostic {
        severity: DiagSeverity::Error,
        message: format!("sv-parser error: {err}"),
        span: SourceSpan::new(file_id, 0, 1, 1, 0),
    })
}

/// Convert any CST node or collection into its trimmed source slice.
pub fn node_text<'a>(source: &'a str, node: impl IntoIterator<Item = RefNode<'a>>) -> &'a str {
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
        if beg <= end && end <= source.len() {
            source[beg..end].trim()
        } else {
            ""
        }
    } else {
        ""
    }
}

/// Compute exact source span (offset, line, column, len) from CST node.
pub fn node_span<'a>(
    file_id: FileId,
    source: &'a str,
    node: impl IntoIterator<Item = RefNode<'a>>,
) -> SourceSpan {
    let mut first_offset = None;
    let mut first_line = 1;
    let mut last_end = 0;

    for n in node {
        if let RefNode::Locate(x) = n {
            if first_offset.is_none() {
                first_offset = Some(x.offset);
                first_line = x.line as u32;
            }
            last_end = x.offset + x.len;
        }
    }

    if let Some(offset) = first_offset {
        let len = if last_end >= offset {
            (last_end - offset) as u32
        } else {
            0
        };
        // Find start of line to compute column
        let line_start = source[..offset.min(source.len())]
            .rfind('\n')
            .map(|idx| idx + 1)
            .unwrap_or(0);
        let column = (offset - line_start + 1) as u32;

        SourceSpan::new(file_id, offset as u32, first_line, column, len)
    } else {
        SourceSpan::new(file_id, 0, 1, 1, 0)
    }
}

/// Extract lightweight summary of definitions and dependencies from a `SyntaxTree`.
pub fn extract_file_summary(file_id: FileId, source: &str, tree: &SyntaxTree) -> FileSummary {
    let mut defs = Vec::new();
    let mut diagnostics = Vec::new();

    for node in tree {
        match node {
            RefNode::ModuleDeclarationAnsi(m) => {
                let header = &m.nodes.0;
                let mod_id_node = &header.nodes.3;
                let name = node_text(source, mod_id_node).to_string();
                let span = node_span(file_id, source, [RefNode::ModuleDeclarationAnsi(m)]);

                if name.is_empty() {
                    diagnostics.push(Diagnostic {
                        severity: DiagSeverity::Error,
                        message: "module declaration missing name".to_string(),
                        span,
                    });
                    continue;
                }

                // Extract instantiated module names
                let mut instantiated = Vec::new();
                for subnode in m {
                    if let RefNode::ModuleInstantiation(mi) = subnode {
                        let target_mod = node_text(source, &mi.nodes.0).to_string();
                        if !target_mod.is_empty() && !instantiated.contains(&target_mod) {
                            instantiated.push(target_mod);
                        }
                    }
                }

                defs.push(DefSummary {
                    name,
                    def_type: DefType::Module,
                    span,
                    instantiated_modules: instantiated,
                });
            }
            RefNode::ModuleDeclarationNonansi(m) => {
                let header = &m.nodes.0;
                let mod_id_node = &header.nodes.3;
                let name = node_text(source, mod_id_node).to_string();
                let span = node_span(file_id, source, [RefNode::ModuleDeclarationNonansi(m)]);

                let mut instantiated = Vec::new();
                for subnode in m {
                    if let RefNode::ModuleInstantiation(mi) = subnode {
                        let target_mod = node_text(source, &mi.nodes.0).to_string();
                        if !target_mod.is_empty() && !instantiated.contains(&target_mod) {
                            instantiated.push(target_mod);
                        }
                    }
                }

                defs.push(DefSummary {
                    name,
                    def_type: DefType::Module,
                    span,
                    instantiated_modules: instantiated,
                });
            }
            RefNode::InterfaceDeclaration(iface) => {
                let span = node_span(file_id, source, [RefNode::InterfaceDeclaration(iface)]);
                let name = node_text(source, [RefNode::InterfaceDeclaration(iface)]).to_string();
                let name = name
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("unknown")
                    .to_string();
                defs.push(DefSummary {
                    name,
                    def_type: DefType::Interface,
                    span,
                    instantiated_modules: Vec::new(),
                });
            }
            RefNode::PackageDeclaration(pkg) => {
                let span = node_span(file_id, source, [RefNode::PackageDeclaration(pkg)]);
                let name = node_text(source, [RefNode::PackageDeclaration(pkg)]).to_string();
                let name = name
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("unknown")
                    .to_string();
                defs.push(DefSummary {
                    name,
                    def_type: DefType::Package,
                    span,
                    instantiated_modules: Vec::new(),
                });
            }
            _ => {}
        }
    }

    FileSummary {
        file_id,
        defs,
        diagnostics,
    }
}

/// Elaborator that builds a `ModulePlan` from a parsed module CST.
pub struct ModulePlanBuilder<'a> {
    file_id: FileId,
    source: &'a str,
    plan: ModulePlan,
    declared_names: FxHashMap<String, TypeDesc>,
}

impl<'a> ModulePlanBuilder<'a> {
    pub fn new(
        id: ModuleId,
        name: String,
        file_id: FileId,
        span: SourceSpan,
        source: &'a str,
    ) -> Self {
        Self {
            file_id,
            source,
            plan: ModulePlan::new(id, name, file_id, span),
            declared_names: FxHashMap::default(),
        }
    }

    fn text(&self, node: impl IntoIterator<Item = RefNode<'a>>) -> &'a str {
        node_text(self.source, node)
    }

    fn span(&self, node: impl IntoIterator<Item = RefNode<'a>>) -> SourceSpan {
        node_span(self.file_id, self.source, node)
    }

    fn record_diag(
        &mut self,
        severity: DiagSeverity,
        message: impl Into<String>,
        span: SourceSpan,
    ) {
        self.plan.diagnostics.push(Diagnostic {
            severity,
            message: message.into(),
            span,
        });
    }

    /// Extract range width: e.g. `[7:0]` -> 8.
    fn extract_range_width(&mut self, node: impl IntoIterator<Item = RefNode<'a>>) -> u32 {
        if let Some(RefNode::ConstantRange(range)) = unwrap_node!(node, ConstantRange) {
            let nodes = range.nodes.clone();
            let msb_text = self.text(&nodes.0);
            let lsb_text = self.text(&nodes.2);
            let (msb, _) = parse_number_literal(msb_text);
            let (lsb, _) = parse_number_literal(lsb_text);
            if msb >= lsb {
                (msb - lsb + 1) as u32
            } else {
                (lsb - msb + 1) as u32
            }
        } else {
            1
        }
    }

    /// Build plan for an ANSI module declaration.
    pub fn build_ansi(mut self, m: &'a sv_parser::ModuleDeclarationAnsi) -> ModulePlan {
        let header = &m.nodes.0;

        // 1. Ports
        if let Some(ref port_list) = header.nodes.6 {
            self.lower_ansi_port_list(port_list);
        }

        // 2. Pre-scan declarations in body so identifiers can be used out of order
        for item in &m.nodes.2 {
            self.prescan_module_item(item);
        }

        // 3. Lower body items
        for item in &m.nodes.2 {
            self.lower_module_item(item);
        }

        self.plan
    }

    /// Build plan for non-ANSI module declaration (currently emits located diagnostic).
    pub fn build_nonansi(mut self, m: &'a sv_parser::ModuleDeclarationNonansi) -> ModulePlan {
        let span = self.span([RefNode::ModuleDeclarationNonansi(m)]);
        self.record_diag(
            DiagSeverity::Error,
            "unsupported SystemVerilog construct: non-ANSI module declaration",
            span,
        );
        self.plan
    }

    fn lower_ansi_port_list(&mut self, list_node: &'a sv_parser::ListOfPortDeclarations) {
        let mut prev_dir = PortDir::Input;

        for node in list_node {
            if let RefNode::AnsiPortDeclaration(ansi_port) = node {
                let id_node = unwrap_node!(ansi_port, PortIdentifier);
                let name = if let Some(id) = id_node {
                    self.text(id).to_string()
                } else {
                    self.record_diag(
                        DiagSeverity::Error,
                        "port missing identifier",
                        self.span([RefNode::AnsiPortDeclaration(ansi_port)]),
                    );
                    continue;
                };

                let dir = if let Some(dir_node) = unwrap_node!(ansi_port, PortDirection) {
                    match dir_node {
                        RefNode::PortDirection(sv_parser::PortDirection::Input(_)) => {
                            PortDir::Input
                        }
                        RefNode::PortDirection(sv_parser::PortDirection::Output(_)) => {
                            PortDir::Output
                        }
                        RefNode::PortDirection(sv_parser::PortDirection::Inout(_)) => {
                            PortDir::Inout
                        }
                        RefNode::PortDirection(sv_parser::PortDirection::Ref(_)) => {
                            self.record_diag(
                                DiagSeverity::Error,
                                "unsupported SystemVerilog construct: ref port",
                                self.span([RefNode::AnsiPortDeclaration(ansi_port)]),
                            );
                            prev_dir
                        }
                        _ => prev_dir,
                    }
                } else {
                    prev_dir
                };
                prev_dir = dir;

                let width = self.extract_range_width(ansi_port);
                let span = self.span([RefNode::AnsiPortDeclaration(ansi_port)]);

                let is_clock = (name == "clk" || name == "clock") && width == 1;
                let is_reset = (name == "rst" || name == "reset") && width == 1;

                let ty = if is_clock {
                    TypeDesc::Clock
                } else if is_reset {
                    TypeDesc::Reset
                } else {
                    TypeDesc::Int(width)
                };

                self.declared_names.insert(name.clone(), ty.clone());
                self.plan.ports.push(PortDesc {
                    name,
                    dir,
                    ty,
                    span,
                });
            }
        }
    }

    fn prescan_module_item(&mut self, item: &'a sv_parser::NonPortModuleItem) {
        if let Some(net_decl) = unwrap_node!(item, NetDeclaration) {
            let width = self.extract_range_width(net_decl.clone());
            if let Some(id_node) =
                unwrap_node!(net_decl, NetIdentifier, SimpleIdentifier, EscapedIdentifier)
            {
                let name = self.text(id_node).to_string();
                self.declared_names.insert(name, TypeDesc::Int(width));
            }
        } else if let Some(data_decl) = unwrap_node!(item, DataDeclaration) {
            let width = self.extract_range_width(data_decl.clone());
            if let Some(unpacked) =
                unwrap_node!(data_decl.clone(), UnpackedDimension, VariableDimension)
            {
                let depth = self.extract_range_width(unpacked);
                if let Some(id_node) = unwrap_node!(
                    data_decl,
                    VariableIdentifier,
                    SimpleIdentifier,
                    EscapedIdentifier
                ) {
                    let name = self.text(id_node).to_string();
                    self.declared_names.insert(
                        name,
                        TypeDesc::Mem {
                            elem_width: width,
                            depth: depth as u64,
                        },
                    );
                }
            } else if let Some(id_node) = unwrap_node!(
                data_decl,
                VariableIdentifier,
                SimpleIdentifier,
                EscapedIdentifier
            ) {
                let name = self.text(id_node).to_string();
                self.declared_names.insert(name, TypeDesc::Int(width));
            }
        }
    }

    fn lower_module_item(&mut self, item: &'a sv_parser::NonPortModuleItem) {
        // Continuous assign: assign x = expr;
        if let Some(assign) = unwrap_node!(item, ContinuousAssign) {
            self.lower_continuous_assign(assign);
            return;
        }

        // Net declaration: wire [7:0] w;
        if let Some(net_decl) = unwrap_node!(item, NetDeclaration) {
            self.lower_net_declaration(net_decl);
            return;
        }

        // Data declaration: logic [7:0] l; reg [7:0] r; logic [7:0] mem [0:15];
        if let Some(data_decl) = unwrap_node!(item, DataDeclaration) {
            self.lower_data_declaration(data_decl);
            return;
        }

        // Always blocks
        if let Some(always) = unwrap_node!(item, AlwaysConstruct) {
            self.lower_always_construct(always);
            return;
        }

        // Module instantiation
        if let Some(inst) = unwrap_node!(item, ModuleInstantiation) {
            self.lower_module_instantiation(inst);
            return;
        }

        let span = self.span([RefNode::NonPortModuleItem(item)]);
        let text = self.text([RefNode::NonPortModuleItem(item)]);
        let keyword = text.split_whitespace().next().unwrap_or("module item");
        self.record_diag(
            DiagSeverity::Error,
            format!("unsupported SystemVerilog construct: {keyword}"),
            span,
        );
    }

    fn lower_continuous_assign(&mut self, assign: RefNode<'a>) {
        let span = self.span(assign.clone());
        let (target, expr_node) = match assign {
            RefNode::ContinuousAssign(sv_parser::ContinuousAssign::Net(n)) => {
                if let Some(RefNode::NetAssignment(na)) = unwrap_node!(&n.nodes.3, NetAssignment) {
                    let target = self.text(&na.nodes.0).to_string();
                    (target, RefNode::Expression(&na.nodes.2))
                } else {
                    self.record_diag(
                        DiagSeverity::Error,
                        "continuous assign missing assignment",
                        span,
                    );
                    return;
                }
            }
            RefNode::ContinuousAssign(sv_parser::ContinuousAssign::Variable(v)) => {
                if let Some(RefNode::VariableAssignment(va)) =
                    unwrap_node!(&v.nodes.2, VariableAssignment)
                {
                    let target = self.text(&va.nodes.0).to_string();
                    (target, RefNode::Expression(&va.nodes.2))
                } else {
                    self.record_diag(
                        DiagSeverity::Error,
                        "continuous assign missing assignment",
                        span,
                    );
                    return;
                }
            }
            _ => {
                self.record_diag(DiagSeverity::Error, "expected continuous assignment", span);
                return;
            }
        };

        let expr_id = self.lower_expression(expr_node);
        self.plan.assigns.push(AssignDesc {
            target,
            expr: expr_id,
            span,
        });
    }

    fn lower_net_declaration(&mut self, net_decl: RefNode<'a>) {
        let span = self.span(net_decl.clone());
        let width = self.extract_range_width(net_decl.clone());
        let id_node = unwrap_node!(net_decl, NetIdentifier, SimpleIdentifier, EscapedIdentifier);
        if let Some(id) = id_node {
            let name = self.text(id).to_string();
            let ty = TypeDesc::Int(width);
            self.declared_names.insert(name.clone(), ty.clone());
            self.plan.decls.push(DeclDesc {
                name,
                kind: DeclKind::Wire,
                ty,
                span,
            });
        } else {
            self.record_diag(
                DiagSeverity::Error,
                "net declaration missing identifier",
                span,
            );
        }
    }

    fn lower_data_declaration(&mut self, data_decl: RefNode<'a>) {
        let span = self.span(data_decl.clone());
        let width = self.extract_range_width(data_decl.clone());
        let decl_text = self.text(data_decl.clone());

        // Memory array: unpacked dimension
        if let Some(unpacked) =
            unwrap_node!(data_decl.clone(), UnpackedDimension, VariableDimension)
        {
            let depth = self.extract_range_width(unpacked);
            if depth > 1 {
                if let Some(id) = unwrap_node!(
                    data_decl.clone(),
                    VariableIdentifier,
                    SimpleIdentifier,
                    EscapedIdentifier
                ) {
                    let name = self.text(id).to_string();
                    let ty = TypeDesc::Mem {
                        elem_width: width,
                        depth: depth as u64,
                    };
                    self.declared_names.insert(name.clone(), ty.clone());
                    self.plan.decls.push(DeclDesc {
                        name,
                        kind: DeclKind::Mem,
                        ty,
                        span,
                    });
                    return;
                }
            }
        }

        if let Some(id) = unwrap_node!(
            data_decl,
            VariableIdentifier,
            SimpleIdentifier,
            EscapedIdentifier
        ) {
            let name = self.text(id).to_string();
            let kind = if decl_text.starts_with("reg") {
                DeclKind::Reg
            } else {
                DeclKind::Logic
            };
            let ty = TypeDesc::Int(width);
            self.declared_names.insert(name.clone(), ty.clone());
            self.plan.decls.push(DeclDesc {
                name,
                kind,
                ty,
                span,
            });
        } else {
            self.record_diag(
                DiagSeverity::Error,
                "data declaration missing identifier",
                span,
            );
        }
    }

    fn lower_always_construct(&mut self, always: RefNode<'a>) {
        let span = self.span(always.clone());
        if let RefNode::AlwaysConstruct(ac) = always {
            match ac.nodes.0 {
                sv_parser::AlwaysKeyword::AlwaysComb(_) => {
                    self.lower_always_comb(&ac.nodes.1, span);
                }
                sv_parser::AlwaysKeyword::AlwaysFf(_) => {
                    self.lower_always_ff(&ac.nodes.1, span);
                }
                _ => {
                    self.record_diag(
                        DiagSeverity::Error,
                        "unsupported SystemVerilog construct: always construct",
                        span,
                    );
                }
            }
        }
    }

    fn lower_always_comb(&mut self, stmt: &'a sv_parser::Statement, span: SourceSpan) {
        if let Some((target, expr_node)) = self.extract_assignment(stmt) {
            let expr = self.lower_expression(expr_node);
            self.plan
                .comb_blocks
                .push(AlwaysCombDesc { target, expr, span });
        } else {
            self.record_diag(
                DiagSeverity::Error,
                "cannot find assignment inside always_comb",
                span,
            );
        }
    }

    fn lower_always_ff(&mut self, stmt: &'a sv_parser::Statement, span: SourceSpan) {
        let timing_stmt = unwrap_node!(stmt, ProceduralTimingControlStatement);
        let (clk_name, rst_info) =
            if let Some(RefNode::ProceduralTimingControlStatement(pts)) = timing_stmt {
                self.extract_clock_and_reset(&pts.nodes.0)
            } else {
                self.record_diag(
                    DiagSeverity::Error,
                    "always_ff requires timing control",
                    span,
                );
                return;
            };

        if clk_name.is_empty() {
            self.record_diag(
                DiagSeverity::Error,
                "cannot extract clock from sensitivity list",
                span,
            );
            return;
        }

        // Conditional statement for reset?
        if let Some(cond_stmt) = unwrap_node!(stmt, ConditionalStatement) {
            let (is_async, rst_name, polarity) = if let Some((r_name, r_pol)) = rst_info {
                (true, r_name, r_pol)
            } else if let Some(RefNode::ConditionalStatement(cs)) =
                unwrap_node!(cond_stmt.clone(), CondPredicate)
            {
                let cond_text = self.text(cs).trim_matches(['(', ')']).trim();
                if cond_text.starts_with(['!', '~']) {
                    (
                        false,
                        cond_text.trim_start_matches(['!', '~']).trim().to_string(),
                        "active_low".to_string(),
                    )
                } else {
                    (false, cond_text.to_string(), "active_high".to_string())
                }
            } else {
                self.record_diag(DiagSeverity::Error, "cannot extract reset condition", span);
                return;
            };

            if let Some((target, rst_expr_node, next_expr_node)) = self.extract_if_else(cond_stmt) {
                let rst_val = self.lower_expression(rst_expr_node);
                let next_val = self.lower_expression(next_expr_node);

                self.plan.ff_blocks.push(AlwaysFfDesc {
                    target,
                    clk_name,
                    next_val,
                    reset: Some(ResetDesc {
                        rst_name,
                        polarity,
                        is_async,
                        rst_val,
                    }),
                    span,
                });
            } else {
                self.record_diag(
                    DiagSeverity::Error,
                    "invalid conditional statement in always_ff",
                    span,
                );
            }
        } else if let Some((target, expr_node)) = self.extract_assignment(stmt) {
            let next_val = self.lower_expression(expr_node);
            self.plan.ff_blocks.push(AlwaysFfDesc {
                target,
                clk_name,
                next_val,
                reset: None,
                span,
            });
        } else {
            self.record_diag(
                DiagSeverity::Error,
                "cannot find assignment inside always_ff",
                span,
            );
        }
    }

    fn extract_clock_and_reset(
        &self,
        timing_ctrl: &'a sv_parser::ProceduralTimingControl,
    ) -> (String, Option<(String, String)>) {
        let text = self.text(timing_ctrl);
        let mut clk_name = String::new();
        let mut rst_info = None;

        let parts: Vec<&str> = text.split("or").collect();
        for (idx, part) in parts.iter().enumerate() {
            let trimmed = part
                .trim()
                .trim_start_matches('@')
                .trim()
                .trim_matches(['(', ')']);
            let is_negedge = trimmed.contains("negedge");

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
                } else {
                    "active_high".to_string()
                };
                rst_info = Some((id.to_string(), pol));
            }
        }

        (clk_name, rst_info)
    }

    fn extract_assignment(
        &self,
        stmt: impl IntoIterator<Item = RefNode<'a>>,
    ) -> Option<(String, RefNode<'a>)> {
        match unwrap_node!(
            stmt,
            NonblockingAssignment,
            OperatorAssignment,
            VariableAssignment
        ) {
            Some(RefNode::NonblockingAssignment(a)) => {
                let target = self.text(&a.nodes.0).to_string();
                Some((target, RefNode::Expression(&a.nodes.3)))
            }
            Some(RefNode::OperatorAssignment(a)) => {
                let target = self.text(&a.nodes.0).to_string();
                Some((target, RefNode::Expression(&a.nodes.2)))
            }
            Some(RefNode::VariableAssignment(a)) => {
                let target = self.text(&a.nodes.0).to_string();
                Some((target, RefNode::Expression(&a.nodes.2)))
            }
            _ => None,
        }
    }

    fn extract_if_else(&self, cond: RefNode<'a>) -> Option<(String, RefNode<'a>, RefNode<'a>)> {
        if let RefNode::ConditionalStatement(cs) = cond {
            let (target_rst, rst_expr) = self.extract_assignment(&cs.nodes.3)?;
            if let Some((_, ref else_stmt)) = cs.nodes.5 {
                let (target_next, next_expr) = self.extract_assignment(else_stmt)?;
                if target_rst == target_next {
                    return Some((target_rst, rst_expr, next_expr));
                }
            }
        }
        None
    }

    fn lower_module_instantiation(&mut self, inst: RefNode<'a>) {
        let span = self.span(inst.clone());
        if let RefNode::ModuleInstantiation(mi) = inst {
            let target_mod = self.text(&mi.nodes.0).to_string();

            for hi in mi.nodes.2.contents() {
                let inst_name = self.text(&hi.nodes.0).to_string();
                let mut conns = Vec::new();

                for conn in hi {
                    if let RefNode::OrderedPortConnection(opc) = conn {
                        if let Some(ref expr) = opc.nodes.1 {
                            let expr_id = self.lower_expression(RefNode::Expression(expr));
                            conns.push((None, expr_id));
                        }
                    } else if let RefNode::NamedPortConnection(npc) = conn
                        && let sv_parser::NamedPortConnection::Identifier(id_conn) = npc
                    {
                        let port_name = self.text(&id_conn.nodes.2).to_string();
                        if let Some(ref paren) = id_conn.nodes.3 {
                            if let Some(ref expr) = paren.nodes.1 {
                                let expr_id = self.lower_expression(RefNode::Expression(expr));
                                conns.push((Some(port_name), expr_id));
                            }
                        } else {
                            let ident_expr = self.plan.add_expr(ExprDesc::Ident {
                                name: port_name.clone(),
                                span,
                            });
                            conns.push((Some(port_name), ident_expr));
                        }
                    }
                }

                self.plan.instances.push(InstanceDesc {
                    inst_name,
                    target_mod: target_mod.clone(),
                    port_connections: conns,
                    span,
                });
            }
        }
    }

    /// Lower an expression into an `ExprId` in the plan's arena.
    pub fn lower_expression(&mut self, expr: RefNode<'a>) -> ExprId {
        let span = self.span(expr.clone());

        // Binary: a + b
        if let Some(bin) = unwrap_node!(expr.clone(), ExpressionBinary, ConstantExpressionBinary) {
            let (lhs_node, op, rhs_node) = match bin {
                RefNode::ExpressionBinary(b) => (
                    RefNode::Expression(&b.nodes.0),
                    self.text(&b.nodes.1).to_string(),
                    RefNode::Expression(&b.nodes.3),
                ),
                RefNode::ConstantExpressionBinary(b) => (
                    RefNode::ConstantExpression(&b.nodes.0),
                    self.text(&b.nodes.1).to_string(),
                    RefNode::ConstantExpression(&b.nodes.3),
                ),
                _ => unreachable!(),
            };
            let lhs = self.lower_expression(lhs_node);
            let rhs = self.lower_expression(rhs_node);
            let res_ty = self.infer_expr_type(lhs);
            return self.plan.add_expr(ExprDesc::Binary {
                op,
                lhs,
                rhs,
                res_ty,
                span,
            });
        }

        // Unary: ~a, !a
        if let Some(un) = unwrap_node!(expr.clone(), ExpressionUnary, ConstantExpressionUnary) {
            let (op, op_node) = match un {
                RefNode::ExpressionUnary(u) => (
                    self.text(&u.nodes.0).to_string(),
                    RefNode::Primary(&u.nodes.2),
                ),
                RefNode::ConstantExpressionUnary(u) => (
                    self.text(&u.nodes.0).to_string(),
                    RefNode::ConstantPrimary(&u.nodes.2),
                ),
                _ => unreachable!(),
            };
            let operand = self.lower_expression(op_node);
            let res_ty = self.infer_expr_type(operand);
            return self.plan.add_expr(ExprDesc::Unary {
                op,
                operand,
                res_ty,
                span,
            });
        }

        // Ternary / mux: sel ? a : b
        if let Some(cond) = unwrap_node!(
            expr.clone(),
            ConditionalExpression,
            ConstantExpressionTernary
        ) {
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
                _ => unreachable!(),
            };
            let cond_id = self.lower_expression(pred_node);
            let true_id = self.lower_expression(true_node);
            let false_id = self.lower_expression(false_node);
            let res_ty = self.infer_expr_type(true_id);
            return self.plan.add_expr(ExprDesc::Mux {
                cond: cond_id,
                true_val: true_id,
                false_val: false_id,
                res_ty,
                span,
            });
        }

        // Concat: {a, b}
        if let Some(concat) = unwrap_node!(expr.clone(), Concatenation, ConstantConcatenation) {
            let mut parts = Vec::new();
            let mut total_width = 0;

            let mut expr_nodes = Vec::new();
            match concat {
                RefNode::Concatenation(c) => {
                    for e in c.nodes.0.nodes.1.contents() {
                        expr_nodes.push(RefNode::Expression(e));
                    }
                }
                RefNode::ConstantConcatenation(c) => {
                    for e in c.nodes.0.nodes.1.contents() {
                        expr_nodes.push(RefNode::ConstantExpression(e));
                    }
                }
                _ => {}
            }

            for e in expr_nodes {
                let part_id = self.lower_expression(e);
                let width = self.infer_expr_type(part_id).bit_width();
                total_width += width;
                parts.push(part_id);
            }

            return self.plan.add_expr(ExprDesc::Concat {
                parts,
                res_ty: TypeDesc::Int(total_width.max(1)),
                span,
            });
        }

        // Primary
        if let Some(prim) = unwrap_node!(expr.clone(), Primary, ConstantPrimary) {
            return self.lower_primary(prim);
        }

        // Direct number
        if let Some(num) = unwrap_node!(expr.clone(), Number, IntegralNumber) {
            let num_text = self.text(num);
            let (val, width) = parse_number_literal(num_text);
            return self.plan.add_expr(ExprDesc::Constant { val, width, span });
        }

        // Direct identifier
        if let Some(id) = unwrap_node!(expr.clone(), SimpleIdentifier, EscapedIdentifier) {
            let name = self.text(id).to_string();
            return self.plan.add_expr(ExprDesc::Ident { name, span });
        }

        self.record_diag(
            DiagSeverity::Error,
            format!("unsupported SystemVerilog construct: {}", self.text(expr)),
            span,
        );
        self.plan.add_expr(ExprDesc::Constant {
            val: 0,
            width: 32,
            span,
        })
    }

    fn lower_primary(&mut self, prim: RefNode<'a>) -> ExprId {
        let span = self.span(prim.clone());

        if let Some(inner) = unwrap_node!(prim.clone(), Expression, ConstantExpression) {
            return self.lower_expression(inner);
        }

        if let Some(hier) = unwrap_node!(prim.clone(), PrimaryHierarchical) {
            let id_node = unwrap_node!(hier.clone(), HierarchicalIdentifier, SimpleIdentifier);
            let name = if let Some(id) = id_node {
                self.text(id).to_string()
            } else {
                "unknown".to_string()
            };

            let base_expr = self.plan.add_expr(ExprDesc::Ident { name, span });

            if let Some(select) = unwrap_node!(hier, Select) {
                // Part-select: a[msb:lsb]
                if let Some(range) = unwrap_node!(select.clone(), ConstantRange) {
                    let (msb, lsb) = match range {
                        RefNode::ConstantRange(r) => {
                            let (m, _) = parse_number_literal(self.text(&r.nodes.0));
                            let (l, _) = parse_number_literal(self.text(&r.nodes.2));
                            (m, l)
                        }
                        _ => (0, 0),
                    };
                    let low = lsb.min(msb) as u32;
                    let width = msb.max(lsb) as u32 - low + 1;
                    return self.plan.add_expr(ExprDesc::Slice {
                        base: base_expr,
                        low,
                        width,
                        span,
                    });
                }

                // Bit-select: a[idx]
                let idx_expr = unwrap_node!(select.clone(), BitSelect)
                    .and_then(|bs| unwrap_node!(bs, Expression))
                    .or_else(|| unwrap_node!(select, Expression));

                if let Some(idx_node) = idx_expr {
                    let idx_id = self.lower_expression(idx_node);
                    return self.plan.add_expr(ExprDesc::Index {
                        base: base_expr,
                        index: idx_id,
                        span,
                    });
                }
            }

            return base_expr;
        }

        if let Some(num) = unwrap_node!(prim.clone(), Number, IntegralNumber) {
            let num_text = self.text(num);
            let (val, width) = parse_number_literal(num_text);
            return self.plan.add_expr(ExprDesc::Constant { val, width, span });
        }

        if let Some(id) = unwrap_node!(prim.clone(), SimpleIdentifier, EscapedIdentifier) {
            let name = self.text(id).to_string();
            return self.plan.add_expr(ExprDesc::Ident { name, span });
        }

        self.record_diag(
            DiagSeverity::Error,
            format!("unsupported SystemVerilog construct: {}", self.text(prim)),
            span,
        );
        self.plan.add_expr(ExprDesc::Constant {
            val: 0,
            width: 32,
            span,
        })
    }

    fn infer_expr_type(&self, expr_id: ExprId) -> TypeDesc {
        if let Some(desc) = self.plan.exprs.get(expr_id.0 as usize) {
            match desc {
                ExprDesc::Constant { width, .. } => TypeDesc::Int(*width),
                ExprDesc::Ident { name, .. } => self
                    .declared_names
                    .get(name)
                    .cloned()
                    .unwrap_or(TypeDesc::Int(32)),
                ExprDesc::Binary { res_ty, .. }
                | ExprDesc::Unary { res_ty, .. }
                | ExprDesc::Mux { res_ty, .. }
                | ExprDesc::Concat { res_ty, .. } => res_ty.clone(),
                ExprDesc::Slice { width, .. } => TypeDesc::Int(*width),
                ExprDesc::Index { .. } => TypeDesc::Int(1),
            }
        } else {
            TypeDesc::Int(32)
        }
    }
}

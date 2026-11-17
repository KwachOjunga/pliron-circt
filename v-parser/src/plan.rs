use std::fmt;

/// Dense 32-bit identifier for an input or loaded file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileId(pub u32);

impl fmt::Display for FileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "file#{}", self.0)
    }
}

/// Dense 32-bit identifier for a top-level definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DefId(pub u32);

/// Dense 32-bit identifier for a module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModuleId(pub u32);

/// Dense 32-bit identifier for an expression node in the plan's arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExprId(pub u32);

/// Exact source text location and span for diagnostics and debug info.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceSpan {
    pub file_id: FileId,
    pub offset: u32,
    pub line: u32,
    pub column: u32,
    pub len: u32,
}

impl SourceSpan {
    pub const UNKNOWN: Self = Self {
        file_id: FileId(0),
        offset: 0,
        line: 1,
        column: 1,
        len: 0,
    };

    pub fn new(file_id: FileId, offset: u32, line: u32, column: u32, len: u32) -> Self {
        Self {
            file_id,
            offset,
            line,
            column,
            len,
        }
    }
}

/// Hardware type descriptor used during elaboration without `pliron::Context`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeDesc {
    /// Sized integer bit vector with `N` bits.
    Int(u32),
    /// Dedicated clock signal type (`!seq.clock`).
    Clock,
    /// Dedicated reset signal type (`!seq.reset`).
    Reset,
    /// Unpacked memory array with element bit width and depth.
    Mem { elem_width: u32, depth: u64 },
}

impl TypeDesc {
    pub fn bit_width(&self) -> u32 {
        match self {
            TypeDesc::Int(w) => *w,
            TypeDesc::Clock | TypeDesc::Reset => 1,
            TypeDesc::Mem { elem_width, .. } => *elem_width,
        }
    }
}

/// Direction of a module interface port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortDir {
    Input,
    Output,
    Inout,
}

/// Port descriptor on a module boundary.
#[derive(Debug, Clone)]
pub struct PortDesc {
    pub name: String,
    pub dir: PortDir,
    pub ty: TypeDesc,
    pub span: SourceSpan,
}

/// Declaration kind for internal signals and variables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclKind {
    Logic,
    Wire,
    Reg,
    Mem,
}

/// Internal signal or net declaration descriptor.
#[derive(Debug, Clone)]
pub struct DeclDesc {
    pub name: String,
    pub kind: DeclKind,
    pub ty: TypeDesc,
    pub span: SourceSpan,
}

/// Sized expression descriptor stored in a flat arena.
#[derive(Debug, Clone)]
pub enum ExprDesc {
    Constant {
        val: u64,
        width: u32,
        span: SourceSpan,
    },
    Ident {
        name: String,
        span: SourceSpan,
    },
    Binary {
        op: String,
        lhs: ExprId,
        rhs: ExprId,
        res_ty: TypeDesc,
        span: SourceSpan,
    },
    Unary {
        op: String,
        operand: ExprId,
        res_ty: TypeDesc,
        span: SourceSpan,
    },
    Mux {
        cond: ExprId,
        true_val: ExprId,
        false_val: ExprId,
        res_ty: TypeDesc,
        span: SourceSpan,
    },
    Concat {
        parts: Vec<ExprId>,
        res_ty: TypeDesc,
        span: SourceSpan,
    },
    Slice {
        base: ExprId,
        low: u32,
        width: u32,
        span: SourceSpan,
    },
    Index {
        base: ExprId,
        index: ExprId,
        span: SourceSpan,
    },
}

impl ExprDesc {
    pub fn span(&self) -> SourceSpan {
        match self {
            ExprDesc::Constant { span, .. }
            | ExprDesc::Ident { span, .. }
            | ExprDesc::Binary { span, .. }
            | ExprDesc::Unary { span, .. }
            | ExprDesc::Mux { span, .. }
            | ExprDesc::Concat { span, .. }
            | ExprDesc::Slice { span, .. }
            | ExprDesc::Index { span, .. } => *span,
        }
    }
}

/// Continuous assignment statement descriptor (`assign target = expr;`).
#[derive(Debug, Clone)]
pub struct AssignDesc {
    pub target: String,
    pub expr: ExprId,
    pub span: SourceSpan,
}

/// Procedural combinational block descriptor (`always_comb`).
#[derive(Debug, Clone)]
pub struct AlwaysCombDesc {
    pub target: String,
    pub expr: ExprId,
    pub span: SourceSpan,
}

/// Reset event details for sequential logic.
#[derive(Debug, Clone)]
pub struct ResetDesc {
    pub rst_name: String,
    pub polarity: String, // "active_high" or "active_low"
    pub is_async: bool,
    pub rst_val: ExprId,
}

/// Procedural sequential clock process descriptor (`always_ff`).
#[derive(Debug, Clone)]
pub struct AlwaysFfDesc {
    pub target: String,
    pub clk_name: String,
    pub next_val: ExprId,
    pub reset: Option<ResetDesc>,
    pub span: SourceSpan,
}

/// Module instantiation descriptor (`child_mod inst (.a(x));`).
#[derive(Debug, Clone)]
pub struct InstanceDesc {
    pub inst_name: String,
    pub target_mod: String,
    pub port_connections: Vec<(Option<String>, ExprId)>,
    pub span: SourceSpan,
}

/// Diagnostic severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DiagSeverity {
    Warning,
    Error,
}

/// A compiler diagnostic message with precise source location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: DiagSeverity,
    pub message: String,
    pub span: SourceSpan,
}

impl PartialOrd for Diagnostic {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Diagnostic {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.span
            .file_id
            .cmp(&other.span.file_id)
            .then(self.span.offset.cmp(&other.span.offset))
            .then(self.span.line.cmp(&other.span.line))
            .then(self.span.column.cmp(&other.span.column))
            .then(self.message.cmp(&other.message))
    }
}

/// Flat, self-contained plan for lowering a single SystemVerilog module into Pliron IR.
///
/// Contains zero references to `pliron::context::Context` and implements `Send + Sync`.
#[derive(Debug, Clone)]
pub struct ModulePlan {
    pub id: ModuleId,
    pub name: String,
    pub file_id: FileId,
    pub span: SourceSpan,
    pub ports: Vec<PortDesc>,
    pub decls: Vec<DeclDesc>,
    pub assigns: Vec<AssignDesc>,
    pub comb_blocks: Vec<AlwaysCombDesc>,
    pub ff_blocks: Vec<AlwaysFfDesc>,
    pub instances: Vec<InstanceDesc>,
    pub exprs: Vec<ExprDesc>,
    pub diagnostics: Vec<Diagnostic>,
}

impl ModulePlan {
    pub fn new(id: ModuleId, name: String, file_id: FileId, span: SourceSpan) -> Self {
        Self {
            id,
            name,
            file_id,
            span,
            ports: Vec::new(),
            decls: Vec::new(),
            assigns: Vec::new(),
            comb_blocks: Vec::new(),
            ff_blocks: Vec::new(),
            instances: Vec::new(),
            exprs: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    pub fn add_expr(&mut self, desc: ExprDesc) -> ExprId {
        let id = ExprId(self.exprs.len() as u32);
        self.exprs.push(desc);
        id
    }
}

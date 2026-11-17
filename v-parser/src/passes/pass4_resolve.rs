use crate::passes::pass2_load::LoadedFile;
use crate::passes::pass3_parse::ParsedUnit;
use crate::plan::{DefId, DiagSeverity, Diagnostic, FileId, SourceSpan};
use crate::syntax::DefType;
use rustc_hash::FxHashMap;

/// Resolved top-level definition symbol.
#[derive(Debug, Clone)]
pub struct ResolvedDef {
    pub def_id: DefId,
    pub name: String,
    pub def_type: DefType,
    pub file_id: FileId,
    pub priority: u32,
    pub is_library: bool,
    pub span: SourceSpan,
    pub instantiated_modules: Vec<String>,
}

/// Output of Pass 4: resolved global definitions table and collision diagnostics.
#[derive(Debug, Clone)]
pub struct Pass4Output {
    pub defs: FxHashMap<String, ResolvedDef>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Pass 4: Resolve definitions.
///
/// Inserts every definition name into a resolved map. Handles collisions deterministically:
/// - Duplicate in command-line files -> error diagnostic with both locations.
/// - Command-line vs library file -> command-line file wins.
/// - Library vs library -> lowest priority (earlier on command-line) wins.
pub fn run_pass4(units: &[ParsedUnit], loaded_files: &[LoadedFile]) -> Pass4Output {
    let mut file_priority_map = FxHashMap::default();
    let mut file_is_lib_map = FxHashMap::default();
    for f in loaded_files {
        file_priority_map.insert(f.file_id, f.priority);
        file_is_lib_map.insert(f.file_id, f.is_library);
    }

    let mut defs: FxHashMap<String, ResolvedDef> = FxHashMap::default();
    let mut diagnostics = Vec::new();

    // Flatten all definitions and sort by (priority, span.offset) for deterministic resolution
    let mut all_defs = Vec::new();
    for unit in units {
        let priority = *file_priority_map.get(&unit.file_id).unwrap_or(&0);
        let is_library = *file_is_lib_map.get(&unit.file_id).unwrap_or(&false);

        for def in &unit.summary.defs {
            all_defs.push((def, priority, is_library));
        }
    }

    all_defs.sort_by(|a, b| {
        a.1.cmp(&b.1)
            .then(a.0.span.file_id.cmp(&b.0.span.file_id))
            .then(a.0.span.offset.cmp(&b.0.span.offset))
    });

    for (def, priority, is_library) in all_defs {
        if let Some(existing) = defs.get_mut(&def.name) {
            // Collision handling
            if !is_library && !existing.is_library {
                // Two command-line files defining the same module
                diagnostics.push(Diagnostic {
                    severity: DiagSeverity::Error,
                    message: format!(
                        "duplicate definition of module '{}' (first at {}:{}, also at {}:{})",
                        def.name,
                        existing.span.line,
                        existing.span.column,
                        def.span.line,
                        def.span.column
                    ),
                    span: def.span,
                });
            } else if !is_library && existing.is_library {
                // Command line file overrides library file
                *existing = ResolvedDef {
                    def_id: existing.def_id,
                    name: def.name.clone(),
                    def_type: def.def_type,
                    file_id: def.span.file_id,
                    priority,
                    is_library,
                    span: def.span,
                    instantiated_modules: def.instantiated_modules.clone(),
                };
            } else if is_library && !existing.is_library {
                // Keep the existing command line definition
            } else {
                // Both are library files: lower priority number (earlier) wins
                if priority < existing.priority {
                    *existing = ResolvedDef {
                        def_id: existing.def_id,
                        name: def.name.clone(),
                        def_type: def.def_type,
                        file_id: def.span.file_id,
                        priority,
                        is_library,
                        span: def.span,
                        instantiated_modules: def.instantiated_modules.clone(),
                    };
                }
            }
        } else {
            let def_id = DefId(defs.len() as u32);
            defs.insert(
                def.name.clone(),
                ResolvedDef {
                    def_id,
                    name: def.name.clone(),
                    def_type: def.def_type,
                    file_id: def.span.file_id,
                    priority,
                    is_library,
                    span: def.span,
                    instantiated_modules: def.instantiated_modules.clone(),
                },
            );
        }
    }

    Pass4Output { defs, diagnostics }
}

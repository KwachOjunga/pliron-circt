use crate::passes::pass2_load::{LoadedFile, load_single_file};
use crate::passes::pass3_parse::{ParsedUnit, make_defines_map};
use crate::passes::pass4_resolve::ResolvedDef;
use crate::plan::{DefId, DiagSeverity, Diagnostic, FileId, SourceSpan};
use crate::syntax::{DefType, extract_file_summary, parse_source};
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::VecDeque;
use std::path::PathBuf;

/// Output of Pass 5: list of live module names to elaborate, updated definitions, and diagnostics.
pub struct Pass5Output {
    pub live_modules: Vec<String>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Pass 5: Liveness analysis and lazy library loading.
///
/// Walks instantiation edges starting from top module(s). If an instantiated module is missing,
/// searches `-y` directories and `-v` library files lazily. Reports all unresolved modules at once.
pub fn run_pass5(
    defs: &mut FxHashMap<String, ResolvedDef>,
    top: Option<&str>,
    lib_dirs: &[PathBuf],
    loaded_files: &mut Vec<LoadedFile>,
    units: &mut Vec<ParsedUnit>,
    defines: &[String],
    incdirs: &[PathBuf],
) -> Pass5Output {
    let mut diagnostics = Vec::new();
    let base_defines = make_defines_map(defines);

    // 1. Identify roots
    let roots: Vec<String> = if let Some(top_name) = top {
        if !defs.contains_key(top_name) {
            diagnostics.push(Diagnostic {
                severity: DiagSeverity::Error,
                message: format!("top module '{top_name}' not found"),
                span: SourceSpan::UNKNOWN,
            });
            return Pass5Output {
                live_modules: Vec::new(),
                diagnostics,
            };
        }
        vec![top_name.to_string()]
    } else {
        // Collect all modules that are instantiated by at least one command-line module
        let mut instantiated_set = FxHashSet::default();
        for def in defs.values().filter(|d| !d.is_library) {
            for child in &def.instantiated_modules {
                instantiated_set.insert(child.clone());
            }
        }

        // Roots are command-line modules not instantiated by any other command-line module
        let mut root_candidates: Vec<String> = defs
            .values()
            .filter(|d| !d.is_library && d.def_type == DefType::Module && !instantiated_set.contains(&d.name))
            .map(|d| d.name.clone())
            .collect();

        if root_candidates.is_empty() {
            // Fallback: all non-library modules
            root_candidates = defs
                .values()
                .filter(|d| !d.is_library && d.def_type == DefType::Module)
                .map(|d| d.name.clone())
                .collect();
        }

        root_candidates.sort();
        root_candidates
    };

    // 2. Reachability walk with lazy loading of library files
    let mut live_set = FxHashSet::default();
    let mut visited = FxHashSet::default();
    let mut queue = VecDeque::new();

    for r in &roots {
        queue.push_back(r.clone());
        visited.insert(r.clone());
    }

    while let Some(mod_name) = queue.pop_front() {
        live_set.insert(mod_name.clone());

        let instantiated = if let Some(d) = defs.get(&mod_name) {
            d.instantiated_modules.clone()
        } else {
            Vec::new()
        };

        for child in instantiated {
            if !defs.contains_key(&child) {
                // Try lazy loading from lib_dirs (-y <dir>)
                let mut loaded_lib = false;
                for dir in lib_dirs {
                    for ext in &["v", "sv"] {
                        let candidate = dir.join(format!("{child}.{ext}"));
                        if candidate.is_file() {
                            let file_id = FileId(loaded_files.len() as u32);
                            if let Ok(text) = load_single_file(&candidate) {
                                let lfile = LoadedFile {
                                    file_id,
                                    path: candidate.clone(),
                                    text,
                                    priority: u32::MAX - 1,
                                    is_library: true,
                                };
                                loaded_files.push(lfile.clone());

                                if let Ok((tree, _)) = parse_source(file_id, text, &candidate, &base_defines, incdirs) {
                                    let summary = extract_file_summary(file_id, text, &tree);
                                    diagnostics.extend(summary.diagnostics.clone());

                                    for d in &summary.defs {
                                        if !defs.contains_key(&d.name) {
                                            let def_id = DefId(defs.len() as u32);
                                            defs.insert(
                                                d.name.clone(),
                                                ResolvedDef {
                                                    def_id,
                                                    name: d.name.clone(),
                                                    def_type: d.def_type,
                                                    file_id,
                                                    priority: u32::MAX - 1,
                                                    is_library: true,
                                                    span: d.span,
                                                    instantiated_modules: d.instantiated_modules.clone(),
                                                },
                                            );
                                        }
                                    }

                                    units.push(ParsedUnit {
                                        file_id,
                                        path: candidate.clone(),
                                        tree,
                                        summary,
                                    });
                                    loaded_lib = true;
                                    break;
                                }
                            }
                        }
                    }
                    if loaded_lib {
                        break;
                    }
                }
            }

            if defs.contains_key(&child) {
                if !visited.contains(&child) {
                    visited.insert(child.clone());
                    queue.push_back(child);
                }
            } else {
                let parent_span = defs.get(&mod_name).map(|d| d.span).unwrap_or(SourceSpan::UNKNOWN);
                diagnostics.push(Diagnostic {
                    severity: DiagSeverity::Error,
                    message: format!("unresolved module '{child}' instantiated in module '{mod_name}'"),
                    span: parent_span,
                });
            }
        }
    }

    let mut live_modules: Vec<String> = live_set.into_iter().collect();
    // Sort live modules deterministically by (priority, span.offset, name)
    live_modules.sort_by(|a, b| {
        let def_a = defs.get(a);
        let def_b = defs.get(b);
        match (def_a, def_b) {
            (Some(da), Some(db)) => da
                .priority
                .cmp(&db.priority)
                .then(da.span.file_id.cmp(&db.span.file_id))
                .then(da.span.offset.cmp(&db.span.offset))
                .then(a.cmp(b)),
            _ => a.cmp(b),
        }
    });

    Pass5Output {
        live_modules,
        diagnostics,
    }
}

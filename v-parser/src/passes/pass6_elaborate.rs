use crate::passes::pass2_load::LoadedFile;
use crate::passes::pass3_parse::ParsedUnit;
use crate::passes::pass4_resolve::ResolvedDef;
use crate::plan::{Diagnostic, ModuleId, ModulePlan};
use crate::syntax::ModulePlanBuilder;
use rayon::prelude::*;
use rustc_hash::FxHashMap;
use sv_parser::{RefNode, unwrap_node};

/// Output of Pass 6: elaborated module plans ready for emission, and elaboration diagnostics.
pub struct Pass6Output {
    pub plans: Vec<ModulePlan>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Pass 6: Parallel elaboration per module.
///
/// Converts each live module's CST into a self-contained `ModulePlan` containing
/// resolved ports, declarations, and an arena of sized expressions and statements.
pub fn run_pass6(
    live_modules: &[String],
    defs: &FxHashMap<String, ResolvedDef>,
    units: &[ParsedUnit],
    loaded_files: &[LoadedFile],
) -> Pass6Output {
    // Map FileId -> text
    let mut file_map = FxHashMap::default();
    for f in loaded_files {
        file_map.insert(f.file_id, f.text);
    }

    // Map FileId -> ParsedUnit
    let mut unit_map = FxHashMap::default();
    for u in units {
        unit_map.insert(u.file_id, u);
    }

    // Elaborate in parallel using Rayon
    let plans: Vec<ModulePlan> = live_modules
        .par_iter()
        .enumerate()
        .filter_map(|(idx, mod_name)| {
            let def = defs.get(mod_name)?;
            let unit = unit_map.get(&def.file_id)?;
            let source_text = *file_map.get(&def.file_id)?;

            let builder = ModulePlanBuilder::new(
                ModuleId(idx as u32),
                mod_name.clone(),
                def.file_id,
                def.span,
                source_text,
            );

            // Locate module CST node in the unit's tree
            for node in &unit.tree {
                if let RefNode::ModuleDeclarationAnsi(m) = node {
                    let header = &m.nodes.0;
                    let id_node = &header.nodes.3;
                    let name = crate::syntax::node_text(source_text, id_node);
                    if name == mod_name {
                        return Some(builder.build_ansi(m));
                    }
                } else if let RefNode::ModuleDeclarationNonansi(m) = node {
                    let header = &m.nodes.0;
                    let id_node = &header.nodes.3;
                    let name = crate::syntax::node_text(source_text, id_node);
                    if name == mod_name {
                        return Some(builder.build_nonansi(m));
                    }
                }
            }

            None
        })
        .collect();

    let mut diagnostics = Vec::new();
    for p in &plans {
        diagnostics.extend(p.diagnostics.clone());
    }

    Pass6Output { plans, diagnostics }
}

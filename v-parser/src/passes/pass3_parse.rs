use crate::passes::pass2_load::LoadedFile;
use crate::plan::{Diagnostic, FileId};
use crate::syntax::{FileSummary, extract_file_summary, parse_source};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;
use sv_parser::{Define, DefineText, SyntaxTree};

/// A parsed compilation unit, owning its `SyntaxTree` and plain `FileSummary`.
pub struct ParsedUnit {
    pub file_id: FileId,
    pub path: PathBuf,
    pub tree: SyntaxTree,
    pub summary: FileSummary,
}

/// Output of Pass 3: parsed units and syntax diagnostics.
pub struct Pass3Output {
    pub units: Vec<ParsedUnit>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Build `Define` map from string definitions (e.g. `+define+WIDTH=8`).
pub fn make_defines_map(defines: &[String]) -> HashMap<String, Option<Define>> {
    let mut map = HashMap::new();
    for def in defines {
        let (key, val) = if let Some((k, v)) = def.split_once('=') {
            (k.trim(), Some(v.trim()))
        } else {
            (def.trim(), None)
        };
        let define_obj = if let Some(v) = val {
            Some(Define::new(
                key.to_string(),
                Vec::new(),
                Some(DefineText::new(v.to_string(), None)),
            ))
        } else {
            Some(Define::new(key.to_string(), Vec::new(), None))
        };
        map.insert(key.to_string(), define_obj);
    }
    map
}

/// Pass 3: Parallel parsing per compilation unit.
///
/// Preprocesses and parses each file into a `SyntaxTree` and extracts a `FileSummary`.
/// If `single_unit` is true, runs serially in command-line order, allowing macros to
/// flow between files.
pub fn run_pass3(
    files: &[LoadedFile],
    defines: &[String],
    incdirs: &[PathBuf],
    single_unit: bool,
) -> Pass3Output {
    let base_defines = make_defines_map(defines);

    if single_unit {
        // Serial mode
        let mut units = Vec::with_capacity(files.len());
        let mut diagnostics = Vec::new();
        let mut cur_defines = base_defines;

        for file in files {
            match parse_source(file.file_id, file.text, &file.path, &cur_defines, incdirs) {
                Ok((tree, new_defines)) => {
                    // cur_defines = new_defines;
                    let summary = extract_file_summary(file.file_id, file.text, &tree);
                    diagnostics.extend(summary.diagnostics.clone());
                    units.push(ParsedUnit {
                        file_id: file.file_id,
                        path: file.path.clone(),
                        tree,
                        summary,
                    });
                }
                Err(err) => {
                    diagnostics.push(err);
                }
            }
        }

        Pass3Output { units, diagnostics }
    } else {
        // Parallel mode (default mold-style): each file is an independent compilation unit
        let results: Vec<(Option<ParsedUnit>, Vec<Diagnostic>)> = files
            .par_iter()
            .map(|file| {
                let mut diags = Vec::new();
                match parse_source(file.file_id, file.text, &file.path, &base_defines, incdirs) {
                    Ok((tree, _)) => {
                        let summary = extract_file_summary(file.file_id, file.text, &tree);
                        diags.extend(summary.diagnostics.clone());
                        (
                            Some(ParsedUnit {
                                file_id: file.file_id,
                                path: file.path.clone(),
                                tree,
                                summary,
                            }),
                            diags,
                        )
                    }
                    Err(err) => {
                        diags.push(err);
                        (None, diags)
                    }
                }
            })
            .collect();

        let mut units = Vec::with_capacity(files.len());
        let mut diagnostics = Vec::new();

        for (unit_opt, diags) in results {
            diagnostics.extend(diags);
            if let Some(unit) = unit_opt {
                units.push(unit);
            }
        }

        Pass3Output { units, diagnostics }
    }
}

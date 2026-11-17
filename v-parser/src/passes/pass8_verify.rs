use crate::plan::{DiagSeverity, Diagnostic, SourceSpan};
use pliron::{context::Context, op::verify_op};
use pliron_circt::hw::{ModuleOp, validation::validate_module};

/// Output of Pass 8: verification status and sorted, deduplicated diagnostics.
pub struct Pass8Output {
    pub diagnostics: Vec<Diagnostic>,
    pub has_errors: bool,
}

/// Pass 8: IR verification and diagnostic reporting.
///
/// Runs `verify_op` and `validate_module` on each emitted `ModuleOp`.
/// Sorts all compiler diagnostics deterministically by (file priority, offset) and prints them.
pub fn run_pass8(
    ctx: &Context,
    modules: &[ModuleOp],
    mut diagnostics: Vec<Diagnostic>,
) -> Pass8Output {
    for module in modules {
        if let Err(err) = verify_op(module, ctx) {
            diagnostics.push(Diagnostic {
                severity: DiagSeverity::Error,
                message: format!("module IR verification failed: {err}"),
                span: SourceSpan::UNKNOWN,
            });
        }

        if let Err(err) = validate_module(ctx, module) {
            diagnostics.push(Diagnostic {
                severity: DiagSeverity::Error,
                message: format!("hardware module validation failed: {err}"),
                span: SourceSpan::UNKNOWN,
            });
        }
    }

    // Sort diagnostics deterministically
    diagnostics.sort();
    diagnostics.dedup();

    let has_errors = diagnostics.iter().any(|d| d.severity == DiagSeverity::Error);

    Pass8Output {
        diagnostics,
        has_errors,
    }
}

use crate::passes::*;
use crate::plan::Diagnostic;
use pliron::context::Context;
use pliron_circt::hw::ModuleOp;
use std::time::{Duration, Instant};

/// Timings recorded for each pass in the pipeline.
#[derive(Debug, Clone, Default)]
pub struct DriverTimings {
    pub pass1_cmdline: Duration,
    pub pass2_load: Duration,
    pub pass3_parse: Duration,
    pub pass4_resolve: Duration,
    pub pass5_liveness: Duration,
    pub pass6_elaborate: Duration,
    pub pass7_emit: Duration,
    pub pass8_verify: Duration,
    pub total: Duration,
}

/// Compilation metrics collected across the pipeline.
#[derive(Debug, Clone, Default)]
pub struct DriverStats {
    pub files_loaded: usize,
    pub units_parsed: usize,
    pub definitions_resolved: usize,
    pub live_modules: usize,
    pub modules_emitted: usize,
}

/// The overall outcome of running the mold-style SystemVerilog front-end pipeline.
pub struct DriverResult {
    pub modules: Vec<ModuleOp>,
    pub diagnostics: Vec<Diagnostic>,
    pub timings: DriverTimings,
    pub stats: DriverStats,
    pub has_errors: bool,
}

/// Run the 8-pass mold-style front-end pipeline.
pub fn run_pipeline(ctx: &mut Context, options: &CmdOptions) -> DriverResult {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(options.threads)
        .build()
        .unwrap_or_else(|_| rayon::ThreadPoolBuilder::new().build().unwrap());

    pool.install(|| run_pipeline_internal(ctx, options))
}

fn run_pipeline_internal(ctx: &mut Context, options: &CmdOptions) -> DriverResult {
    let total_start = Instant::now();
    let mut timings = DriverTimings::default();
    let mut stats = DriverStats::default();
    let mut all_diagnostics = Vec::new();

    // -------------------------------------------------------------------------
    // Pass 1: Command line and file lists
    // -------------------------------------------------------------------------
    let p1_start = Instant::now();
    let p1_out = match run_pass1(options) {
        Ok(out) => out,
        Err(e) => {
            return DriverResult {
                modules: Vec::new(),
                diagnostics: vec![crate::plan::Diagnostic {
                    severity: crate::plan::DiagSeverity::Error,
                    message: format!("pass 1 command-line processing error: {e}"),
                    span: crate::plan::SourceSpan::UNKNOWN,
                }],
                timings,
                stats,
                has_errors: true,
            };
        }
    };
    timings.pass1_cmdline = p1_start.elapsed();

    // -------------------------------------------------------------------------
    // Pass 2: Load and map files
    // -------------------------------------------------------------------------
    let p2_start = Instant::now();
    let mut p2_out = run_pass2(&p1_out.file_entries);
    timings.pass2_load = p2_start.elapsed();
    all_diagnostics.extend(p2_out.diagnostics);
    stats.files_loaded = p2_out.files.len();

    // -------------------------------------------------------------------------
    // Pass 3: Parallel parse per compilation unit
    // -------------------------------------------------------------------------
    let p3_start = Instant::now();
    let mut p3_out = run_pass3(
        &p2_out.files,
        &p1_out.defines,
        &p1_out.incdirs,
        p1_out.single_unit,
    );
    timings.pass3_parse = p3_start.elapsed();
    all_diagnostics.extend(p3_out.diagnostics);
    stats.units_parsed = p3_out.units.len();

    // -------------------------------------------------------------------------
    // Pass 4: Resolve definitions and detect collisions
    // -------------------------------------------------------------------------
    let p4_start = Instant::now();
    let p4_out = run_pass4(&p3_out.units, &p2_out.files);
    timings.pass4_resolve = p4_start.elapsed();
    all_diagnostics.extend(p4_out.diagnostics);
    stats.definitions_resolved = p4_out.defs.len();
    let mut defs = p4_out.defs;

    // -------------------------------------------------------------------------
    // Pass 5: Liveness analysis and lazy library loading
    // -------------------------------------------------------------------------
    let p5_start = Instant::now();
    let p5_out = run_pass5(
        &mut defs,
        p1_out.top.as_deref(),
        &p1_out.lib_dirs,
        &mut p2_out.files,
        &mut p3_out.units,
        &p1_out.defines,
        &p1_out.incdirs,
    );
    timings.pass5_liveness = p5_start.elapsed();
    all_diagnostics.extend(p5_out.diagnostics);
    stats.live_modules = p5_out.live_modules.len();

    // -------------------------------------------------------------------------
    // Pass 6: Parallel elaboration per module
    // -------------------------------------------------------------------------
    let p6_start = Instant::now();
    let p6_out = run_pass6(
        &p5_out.live_modules,
        &defs,
        &p3_out.units,
        &p2_out.files,
    );
    timings.pass6_elaborate = p6_start.elapsed();
    all_diagnostics.extend(p6_out.diagnostics);

    // -------------------------------------------------------------------------
    // Pass 7: Serial deterministic emission into Context
    // -------------------------------------------------------------------------
    let p7_start = Instant::now();
    let p7_out = run_pass7(ctx, p6_out.plans);
    timings.pass7_emit = p7_start.elapsed();
    stats.modules_emitted = p7_out.modules.len();

    // -------------------------------------------------------------------------
    // Pass 8: Verification and diagnostic sorting
    // -------------------------------------------------------------------------
    let p8_start = Instant::now();
    let p8_out = run_pass8(ctx, &p7_out.modules, all_diagnostics);
    timings.pass8_verify = p8_start.elapsed();
    timings.total = total_start.elapsed();

    DriverResult {
        modules: p7_out.modules,
        diagnostics: p8_out.diagnostics,
        timings,
        stats,
        has_errors: p8_out.has_errors,
    }
}

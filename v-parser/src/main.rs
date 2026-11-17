//! Primary CLI executable for the `v-parser` SystemVerilog front-end compiler.

use clap::Parser;
use pliron::{context::Context, printable::Printable};
use pliron_circt::sv::printer::render_module;
use std::path::PathBuf;
use std::process::ExitCode;
use v_parser::driver::{DriverResult, run_pipeline};
use v_parser::passes::CmdOptions;
use v_parser::plan::DiagSeverity;

#[derive(Parser, Debug)]
#[command(
    name = "v-parser",
    author = "Reginald Ojunga",
    version = "0.1.0",
    about = "SystemVerilog front-end compiler targeting the hw, sv, and seq dialects"
)]
pub struct CliArgs {
    /// Input SystemVerilog source files
    #[arg(value_name = "FILE")]
    pub files: Vec<PathBuf>,

    /// Command file list (-f filelist)
    #[arg(short = 'f', long = "filelist", value_name = "FILE")]
    pub file_lists: Vec<PathBuf>,

    /// Library directories (-y dir)
    #[arg(short = 'y', value_name = "DIR")]
    pub lib_dirs: Vec<PathBuf>,

    /// Library files (-v file)
    #[arg(short = 'v', value_name = "FILE")]
    pub lib_files: Vec<PathBuf>,

    /// Include directories (+incdir+dir or -I dir)
    #[arg(short = 'I', long = "incdir", value_name = "DIR")]
    pub incdirs: Vec<PathBuf>,

    /// Macro definitions (+define+key=val or -D key=val)
    #[arg(short = 'D', long = "define", value_name = "MACRO")]
    pub defines: Vec<String>,

    /// Top module name
    #[arg(long = "top", value_name = "MODULE")]
    pub top: Option<String>,

    /// Number of worker threads (default: available cores up to 32)
    #[arg(long = "threads")]
    pub threads: Option<usize>,

    /// Print performance and pass timing breakdown
    #[arg(long = "perf")]
    pub perf: bool,

    /// Print pipeline compilation metrics
    #[arg(long = "stats")]
    pub stats: bool,

    /// Single compilation unit mode (sequential macro propagation)
    #[arg(long = "single-unit")]
    pub single_unit: bool,

    /// Output file path
    #[arg(short = 'o', long = "output", value_name = "OUT")]
    pub output: Option<PathBuf>,

    /// Print emitted IR
    #[arg(long = "print-ir")]
    pub print_ir: bool,
}

fn normalize_args() -> Vec<String> {
    let mut normalized = Vec::new();
    for arg in std::env::args() {
        if let Some(inc) = arg.strip_prefix("+incdir+") {
            for part in inc.split('+') {
                normalized.push("-I".to_string());
                normalized.push(part.to_string());
            }
        } else if let Some(def) = arg.strip_prefix("+define+") {
            normalized.push("-D".to_string());
            normalized.push(def.to_string());
        } else {
            normalized.push(arg);
        }
    }
    normalized
}

pub fn main() -> ExitCode {
    let args = CliArgs::parse_from(normalize_args());

    let num_threads = args
        .threads
        .unwrap_or_else(|| num_cpus::get().min(32))
        .max(1);

    let options = CmdOptions {
        files: args.files,
        file_lists: args.file_lists,
        incdirs: args.incdirs,
        defines: args.defines,
        lib_dirs: args.lib_dirs,
        lib_files: args.lib_files,
        top: args.top,
        threads: num_threads,
        perf: args.perf,
        stats: args.stats,
        single_unit: args.single_unit,
        output_file: args.output.clone(),
    };

    let mut ctx = Context::new();
    let res: DriverResult = run_pipeline(&mut ctx, &options);

    // Diagnostics reporting
    for diag in &res.diagnostics {
        let prefix = match diag.severity {
            DiagSeverity::Error => "\x1b[1;31merror\x1b[0m",
            DiagSeverity::Warning => "\x1b[1;33mwarning\x1b[0m",
        };
        eprintln!(
            "{}:{}:{}: {}: {}",
            diag.span.file_id, diag.span.line, diag.span.column, prefix, diag.message
        );
    }

    // Performance output
    if options.perf {
        println!("--- Compilation Performance Timings ---");
        println!("Pass 1 (Command Line): {:?}", res.timings.pass1_cmdline);
        println!("Pass 2 (Load & Map)  : {:?}", res.timings.pass2_load);
        println!("Pass 3 (Parse)       : {:?}", res.timings.pass3_parse);
        println!("Pass 4 (Resolve)     : {:?}", res.timings.pass4_resolve);
        println!("Pass 5 (Liveness)    : {:?}", res.timings.pass5_liveness);
        println!("Pass 6 (Elaborate)   : {:?}", res.timings.pass6_elaborate);
        println!("Pass 7 (Emit)        : {:?}", res.timings.pass7_emit);
        println!("Pass 8 (Verify)      : {:?}", res.timings.pass8_verify);
        println!("Total Time           : {:?}", res.timings.total);
    }

    // Metrics output
    if options.stats {
        println!("--- Compilation Metrics ---");
        println!("Files loaded        : {}", res.stats.files_loaded);
        println!("Units parsed        : {}", res.stats.units_parsed);
        println!("Definitions resolved: {}", res.stats.definitions_resolved);
        println!("Live modules        : {}", res.stats.live_modules);
        println!("Modules emitted     : {}", res.stats.modules_emitted);
    }

    // Output generation
    let mut rendered_output = String::new();
    for module in &res.modules {
        if args.print_ir {
            rendered_output.push_str(&module.get_operation().disp(&ctx).to_string());
            rendered_output.push('\n');
        } else if let Ok(rendered) = render_module(&ctx, module) {
            rendered_output.push_str(&rendered);
            rendered_output.push('\n');
        }
    }

    if let Some(ref out_path) = options.output_file {
        if let Err(e) = std::fs::write(out_path, &rendered_output) {
            eprintln!("error writing output to '{}': {e}", out_path.display());
            return ExitCode::FAILURE;
        }
    } else if !rendered_output.is_empty() && (args.print_ir || !res.modules.is_empty()) {
        print!("{rendered_output}");
    }

    if res.has_errors {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

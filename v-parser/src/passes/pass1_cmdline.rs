use std::path::{Path, PathBuf};

/// Input configuration parsed from CLI flags and filelists.
#[derive(Debug, Clone)]
pub struct CmdOptions {
    pub files: Vec<PathBuf>,
    pub file_lists: Vec<PathBuf>,
    pub incdirs: Vec<PathBuf>,
    pub defines: Vec<String>,
    pub lib_dirs: Vec<PathBuf>,
    pub lib_files: Vec<PathBuf>,
    pub top: Option<String>,
    pub threads: usize,
    pub perf: bool,
    pub stats: bool,
    pub single_unit: bool,
    pub output_file: Option<PathBuf>,
}

impl Default for CmdOptions {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            file_lists: Vec::new(),
            incdirs: Vec::new(),
            defines: Vec::new(),
            lib_dirs: Vec::new(),
            lib_files: Vec::new(),
            top: None,
            threads: num_cpus::get().min(32),
            perf: false,
            stats: false,
            single_unit: false,
            output_file: None,
        }
    }
}

/// A source or library file discovered with its deterministic priority index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub path: PathBuf,
    pub priority: u32,
    pub is_library: bool,
}

/// Result of Pass 1: fully expanded, ordered file entries and preprocessor configuration.
#[derive(Debug, Clone)]
pub struct Pass1Output {
    pub file_entries: Vec<FileEntry>,
    pub incdirs: Vec<PathBuf>,
    pub defines: Vec<String>,
    pub lib_dirs: Vec<PathBuf>,
    pub lib_files: Vec<PathBuf>,
    pub top: Option<String>,
    pub threads: usize,
    pub perf: bool,
    pub stats: bool,
    pub single_unit: bool,
    pub output_file: Option<PathBuf>,
}

/// Pass 1: Command line and file list processing.
///
/// Expands `-f` file lists recursively, resolves relative paths, parses `+incdir+`
/// and `+define+`, and assigns each file a deterministic priority equal to its order.
pub fn run_pass1(options: &CmdOptions) -> std::io::Result<Pass1Output> {
    let mut file_entries = Vec::new();
    let mut incdirs = options.incdirs.clone();
    let mut defines = options.defines.clone();
    let mut lib_dirs = options.lib_dirs.clone();
    let mut lib_files = options.lib_files.clone();
    let mut cur_priority = 0u32;

    // Helper closure to process tokens from command line or file lists
    fn process_token(
        token: &str,
        base_dir: &Path,
        file_entries: &mut Vec<FileEntry>,
        incdirs: &mut Vec<PathBuf>,
        defines: &mut Vec<String>,
        lib_dirs: &mut Vec<PathBuf>,
        lib_files: &mut Vec<PathBuf>,
        cur_priority: &mut u32,
        visited_lists: &mut Vec<PathBuf>,
    ) -> std::io::Result<()> {
        let t = token.trim();
        if t.is_empty() || t.starts_with('#') || t.starts_with("//") {
            return Ok(());
        }

        if let Some(inc) = t.strip_prefix("+incdir+") {
            for part in inc.split('+') {
                let p = Path::new(part);
                let full = if p.is_relative() {
                    base_dir.join(p)
                } else {
                    p.to_path_buf()
                };
                incdirs.push(full);
            }
        } else if let Some(def) = t.strip_prefix("+define+") {
            defines.push(def.to_string());
        } else if let Some(y) = t.strip_prefix("-y") {
            let path_str = y.trim();
            if !path_str.is_empty() {
                let p = Path::new(path_str);
                let full = if p.is_relative() {
                    base_dir.join(p)
                } else {
                    p.to_path_buf()
                };
                lib_dirs.push(full);
            }
        } else if let Some(v) = t.strip_prefix("-v") {
            let path_str = v.trim();
            if !path_str.is_empty() {
                let p = Path::new(path_str);
                let full = if p.is_relative() {
                    base_dir.join(p)
                } else {
                    p.to_path_buf()
                };
                lib_files.push(full);
            }
        } else if t == "-f" || t == "-F" {
            // Next token handled in caller if separate, or flag
        } else {
            // Check if it's a file
            let p = Path::new(t);
            let full = if p.is_relative() {
                base_dir.join(p)
            } else {
                p.to_path_buf()
            };
            file_entries.push(FileEntry {
                path: full,
                priority: *cur_priority,
                is_library: false,
            });
            *cur_priority += 1;
        }
        Ok(())
    }

    // Helper to read and expand `-f` file list
    fn expand_file_list(
        list_path: &Path,
        file_entries: &mut Vec<FileEntry>,
        incdirs: &mut Vec<PathBuf>,
        defines: &mut Vec<String>,
        lib_dirs: &mut Vec<PathBuf>,
        lib_files: &mut Vec<PathBuf>,
        cur_priority: &mut u32,
        visited_lists: &mut Vec<PathBuf>,
    ) -> std::io::Result<()> {
        let canonical = list_path
            .canonicalize()
            .unwrap_or_else(|_| list_path.to_path_buf());
        if visited_lists.contains(&canonical) {
            return Ok(()); // Avoid infinite recursion
        }
        visited_lists.push(canonical);

        let content = std::fs::read_to_string(list_path)?;
        let base_dir = list_path.parent().unwrap_or_else(|| Path::new("."));

        let mut tokens = content.split_whitespace().peekable();
        while let Some(tok) = tokens.next() {
            if tok == "-f" || tok == "-F" {
                if let Some(nested) = tokens.next() {
                    let nested_p = Path::new(nested);
                    let full_nested = if nested_p.is_relative() {
                        base_dir.join(nested_p)
                    } else {
                        nested_p.to_path_buf()
                    };
                    expand_file_list(
                        &full_nested,
                        file_entries,
                        incdirs,
                        defines,
                        lib_dirs,
                        lib_files,
                        cur_priority,
                        visited_lists,
                    )?;
                }
            } else if tok == "-y" {
                if let Some(dir) = tokens.next() {
                    let p = Path::new(dir);
                    let full = if p.is_relative() {
                        base_dir.join(p)
                    } else {
                        p.to_path_buf()
                    };
                    lib_dirs.push(full);
                }
            } else if tok == "-v" {
                if let Some(file) = tokens.next() {
                    let p = Path::new(file);
                    let full = if p.is_relative() {
                        base_dir.join(p)
                    } else {
                        p.to_path_buf()
                    };
                    lib_files.push(full);
                }
            } else {
                process_token(
                    tok,
                    base_dir,
                    file_entries,
                    incdirs,
                    defines,
                    lib_dirs,
                    lib_files,
                    cur_priority,
                    visited_lists,
                )?;
            }
        }

        Ok(())
    }

    let mut visited = Vec::new();

    // Process explicit files
    for file in &options.files {
        file_entries.push(FileEntry {
            path: file.clone(),
            priority: cur_priority,
            is_library: false,
        });
        cur_priority += 1;
    }

    // Process file lists
    for flist in &options.file_lists {
        expand_file_list(
            flist,
            &mut file_entries,
            &mut incdirs,
            &mut defines,
            &mut lib_dirs,
            &mut lib_files,
            &mut cur_priority,
            &mut visited,
        )?;
    }

    // Add explicit -v library files with high priority number (lower precedence)
    for lib_file in &options.lib_files {
        if !file_entries.iter().any(|e| e.path == *lib_file) {
            file_entries.push(FileEntry {
                path: lib_file.clone(),
                priority: cur_priority,
                is_library: true,
            });
            cur_priority += 1;
        }
    }

    Ok(Pass1Output {
        file_entries,
        incdirs,
        defines,
        lib_dirs,
        lib_files,
        top: options.top.clone(),
        threads: options.threads.max(1),
        perf: options.perf,
        stats: options.stats,
        single_unit: options.single_unit,
        output_file: options.output_file.clone(),
    })
}

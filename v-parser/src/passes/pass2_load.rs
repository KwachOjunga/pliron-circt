use crate::passes::pass1_cmdline::FileEntry;
use crate::plan::{DiagSeverity, Diagnostic, FileId, SourceSpan};
use std::fs::File;
use std::path::{Path, PathBuf};

/// A file that has been memory-mapped or read, with its bytes leaked to `'static`.
#[derive(Debug, Clone)]
pub struct LoadedFile {
    pub file_id: FileId,
    pub path: PathBuf,
    pub text: &'static str,
    pub priority: u32,
    pub is_library: bool,
}

/// Output of Pass 2: all memory-mapped input files and any file-loading diagnostics.
#[derive(Debug, Clone)]
pub struct Pass2Output {
    pub files: Vec<LoadedFile>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Pass 2: Load and map files.
///
/// Memory-maps every input file with `memmap2`, leaks the buffer to `'static`,
/// and assigns dense sequential `FileId`s.
pub fn run_pass2(entries: &[FileEntry]) -> Pass2Output {
    let mut files = Vec::with_capacity(entries.len());
    let mut diagnostics = Vec::new();

    for (idx, entry) in entries.iter().enumerate() {
        let file_id = FileId(idx as u32);
        match load_single_file(&entry.path) {
            Ok(text) => {
                files.push(LoadedFile {
                    file_id,
                    path: entry.path.clone(),
                    text,
                    priority: entry.priority,
                    is_library: entry.is_library,
                });
            }
            Err(err) => {
                diagnostics.push(Diagnostic {
                    severity: DiagSeverity::Error,
                    message: format!("cannot load file '{}': {err}", entry.path.display()),
                    span: SourceSpan::new(file_id, 0, 1, 1, 0),
                });
            }
        }
    }

    Pass2Output { files, diagnostics }
}

/// Memory-map or read a single file, leaking its contents to `&'static str`.
pub fn load_single_file(path: &Path) -> std::io::Result<&'static str> {
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    let len = metadata.len();

    if len == 0 {
        return Ok("");
    }

    // Memory-map the file
    let mmap = unsafe { memmap2::Mmap::map(&file)? };
    let bytes = Box::leak(mmap.to_vec().into_boxed_slice());
    std::str::from_utf8(bytes).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("file contains invalid UTF-8: {e}"),
        )
    })
}

/// Load an in-memory string directly into a `'static` slice.
pub fn load_in_memory_str(source: &str, file_id: FileId, priority: u32) -> LoadedFile {
    let leaked: &'static str = Box::leak(source.to_string().into_boxed_str());
    LoadedFile {
        file_id,
        path: PathBuf::from("<in-memory>"),
        text: leaked,
        priority,
        is_library: false,
    }
}

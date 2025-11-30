use memmap2::Mmap;
use rayon::prelude::*;
use shared::types::Result;
use shared::utils::is_supported_file;
use std::fs::File;
use std::path::{Path, PathBuf};

pub struct FileScanner {
    root_path: PathBuf,
    ignored_dirs: Vec<String>,
    max_file_bytes: u64,
}

impl FileScanner {
    pub fn new(root_path: impl Into<PathBuf>) -> Self {
        Self {
            root_path: root_path.into(),
            ignored_dirs: vec![
                ".git".into(),
                "target".into(),
                "node_modules".into(),
                ".next".into(),
                "dist".into(),
                "build".into(),
                ".idea".into(),
                ".vscode".into(),
                ".cache".into(),
                "venv".into(),
                "__pycache__".into(),
            ],
            // Cap per-file scanning to keep indexing responsive; adjust if needed.
            max_file_bytes: 2 * 1024 * 1024,
        }
    }

    pub fn scan_files(&self) -> Result<Vec<FileChunk>> {
        let files = self.collect_files()?;
        self.scan_paths(&files)
    }

    pub fn scan_paths(&self, paths: &[PathBuf]) -> Result<Vec<FileChunk>> {
        let chunks: Vec<Result<Vec<FileChunk>>> = paths
            .par_iter()
            .map(|path| self.load_and_chunk_file(path))
            .collect();
        let mut all_chunks = Vec::new();
        for chunk_result in chunks {
            all_chunks.extend(chunk_result?);
        }
        Ok(all_chunks)
    }

    pub fn collect_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        self.collect_files_recursive(&self.root_path, &mut files)?;
        Ok(files)
    }

    /// Return a compact directory overview for context (limited depth/entries).
    pub fn directory_overview(&self, max_depth: usize, max_entries: usize) -> String {
        let mut lines = Vec::new();
        self.walk_directory(
            &self.root_path,
            &mut lines,
            0,
            max_depth,
            max_entries,
            &mut 0,
        );
        lines.join("\n")
    }

    fn walk_directory(
        &self,
        dir: &Path,
        lines: &mut Vec<String>,
        depth: usize,
        max_depth: usize,
        max_entries: usize,
        seen: &mut usize,
    ) {
        if depth > max_depth || *seen >= max_entries {
            return;
        }

        let rel = dir
            .strip_prefix(&self.root_path)
            .unwrap_or(dir)
            .to_string_lossy()
            .to_string();
        let indent = "  ".repeat(depth);
        lines.push(format!(
            "{}{}",
            indent,
            if rel.is_empty() { "." } else { &rel }
        ));
        *seen += 1;
        if *seen >= max_entries {
            return;
        }

        let Ok(read_dir) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if self.ignored_dirs.iter().any(|i| i == name) {
                        continue;
                    }
                }
                self.walk_directory(&path, lines, depth + 1, max_depth, max_entries, seen);
                if *seen >= max_entries {
                    return;
                }
            }
        }
    }

    fn collect_files_recursive(&self, dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if self.ignored_dirs.iter().any(|i| i == name) {
                        continue;
                    }
                }
                self.collect_files_recursive(&path, files)?;
            } else if is_supported_file(&path) {
                files.push(path);
            }
        }
        Ok(())
    }

    fn load_and_chunk_file(&self, path: &Path) -> Result<Vec<FileChunk>> {
        if let Ok(meta) = path.metadata() {
            if meta.len() > self.max_file_bytes {
                return Ok(Vec::new());
            }
        }
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };
        // Lossy conversion ensures non-UTF8 bytes don't crash scanning.
        let content = String::from_utf8_lossy(&mmap).into_owned();
        Ok(self.chunk_text(&content, path))
    }

    fn chunk_text(&self, text: &str, path: &Path) -> Vec<FileChunk> {
        const CHUNK_SIZE: usize = 1000;
        const OVERLAP: usize = 200;

        let mut chunks = Vec::new();
        let mut start = 0;
        let path_str = path.to_string_lossy().to_string();

        while start < text.len() {
            let mut end = (start + CHUNK_SIZE).min(text.len());
            // Ensure we cut on UTF-8 boundaries
            while end < text.len() && !text.is_char_boundary(end) {
                end += 1;
            }
            let chunk_text = text[start..end].to_string();
            chunks.push(FileChunk {
                path: path_str.clone(),
                text: chunk_text,
                start_offset: start,
            });

            if end == text.len() {
                break;
            }
            let mut next_start = end.saturating_sub(OVERLAP);
            while next_start > 0 && !text.is_char_boundary(next_start) {
                next_start -= 1;
            }
            start = next_start;
        }
        chunks
    }
}

#[derive(Debug, Clone)]
pub struct FileChunk {
    pub path: String,
    pub text: String,
    pub start_offset: usize,
}

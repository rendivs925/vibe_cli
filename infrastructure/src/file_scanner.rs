use md5;
use memmap2::Mmap;
use rayon::prelude::*;
use shared::types::Result;
use shared::utils::is_supported_file;
use std::collections::HashSet;
use std::fs::File;
use std::path::{Path, PathBuf};

pub struct FileScanner {
    root_path: PathBuf,
    ignored_dirs: HashSet<String>,
    max_file_bytes: u64,
}

impl FileScanner {
    pub fn new(root_path: impl Into<PathBuf>) -> Self {
        Self {
            root_path: root_path.into(),
            ignored_dirs: [
                ".git",
                "target",
                "node_modules",
                ".next",
                "dist",
                "build",
                ".idea",
                ".vscode",
                ".cache",
                "venv",
                "__pycache__",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            // Cap per-file scanning to keep indexing responsive; adjust if needed.
            max_file_bytes: 2 * 1024 * 1024,
        }
    }

    pub fn scan_files(&self) -> Result<Vec<FileScanResult>> {
        let files = self.collect_files()?;
        self.scan_paths(&files)
    }

    pub fn scan_paths(&self, paths: &[PathBuf]) -> Result<Vec<FileScanResult>> {
        eprintln!("Scanning files with parallel processing...");
        let mut all_results = Vec::with_capacity(paths.len());
        let results: Vec<Result<FileScanResult>> = paths
            .par_iter()
            .map(|path| self.load_and_chunk_file(path))
            .collect();
        for res in results {
            all_results.push(res?);
        }
        Ok(all_results)
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
                    if self.ignored_dirs.contains(name) {
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
                    if self.ignored_dirs.contains(name) {
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

    fn load_and_chunk_file(&self, path: &Path) -> Result<FileScanResult> {
        if let Ok(meta) = path.metadata() {
            if meta.len() > self.max_file_bytes {
                return Ok(FileScanResult {
                    path: path.to_string_lossy().to_string(),
                    hash: String::new(),
                    chunks: Vec::new(),
                });
            }
        }
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };
        // Lossy conversion ensures non-UTF8 bytes don't crash scanning.
        let content = String::from_utf8_lossy(&mmap).into_owned();
        let hash = format!("{:x}", md5::compute(content.as_bytes()));
        let chunks = self.chunk_text(&content, path);
        Ok(FileScanResult {
            path: path.to_string_lossy().to_string(),
            hash,
            chunks,
        })
    }

    fn chunk_text(&self, text: &str, path: &Path) -> Vec<FileChunk> {
        const MAX_CHUNK_SIZE: usize = 2000;
        const MIN_CHUNK_SIZE: usize = 500;

        let mut chunks = Vec::new();
        let mut seen_hashes = HashSet::new();
        let path_str = path.to_string_lossy().to_string();

        // Split text into paragraphs (double newlines)
        let paragraphs: Vec<&str> = text.split("\n\n").collect();
        let mut current_chunk = String::new();
        let mut start_offset = 0;

        for paragraph in paragraphs {
            if current_chunk.len() + paragraph.len() > MAX_CHUNK_SIZE && !current_chunk.is_empty() {
                // Check deduplication
                let hash = format!("{:x}", md5::compute(current_chunk.as_bytes()));
                if seen_hashes.insert(hash) {
                    chunks.push(FileChunk {
                        path: path_str.clone(),
                        text: current_chunk.clone(),
                        start_offset,
                    });
                }
                current_chunk.clear();
                start_offset += paragraph.as_ptr() as usize - text.as_ptr() as usize;
            }

            if !current_chunk.is_empty() {
                current_chunk.push_str("\n\n");
            }
            current_chunk.push_str(paragraph);

            if current_chunk.len() >= MIN_CHUNK_SIZE {
                let hash = format!("{:x}", md5::compute(current_chunk.as_bytes()));
                if seen_hashes.insert(hash) {
                    chunks.push(FileChunk {
                        path: path_str.clone(),
                        text: current_chunk.clone(),
                        start_offset,
                    });
                }
                current_chunk.clear();
                start_offset += paragraph.as_ptr() as usize - text.as_ptr() as usize + paragraph.len();
            }
        }

        // Add remaining chunk
        if !current_chunk.is_empty() {
            let hash = format!("{:x}", md5::compute(current_chunk.as_bytes()));
            if seen_hashes.insert(hash) {
                chunks.push(FileChunk {
                    path: path_str.clone(),
                    text: current_chunk,
                    start_offset,
                });
            }
        }

        // If no chunks, fallback to fixed size
        if chunks.is_empty() {
            self.chunk_fixed_size_dedup(text, path)
        } else {
            chunks
        }
    }

    fn chunk_fixed_size_dedup(&self, text: &str, path: &Path) -> Vec<FileChunk> {
        const CHUNK_SIZE: usize = 1000;
        const OVERLAP: usize = 200;

        let mut chunks = Vec::new();
        let mut seen_hashes = HashSet::new();
        let mut start = 0;
        let path_str = path.to_string_lossy().to_string();
        let estimated = (text.len() / (CHUNK_SIZE.saturating_sub(OVERLAP)).max(1)) + 2;
        chunks.reserve(estimated);

        while start < text.len() {
            let mut end = (start + CHUNK_SIZE).min(text.len());
            // Ensure we cut on UTF-8 boundaries
            while end < text.len() && !text.is_char_boundary(end) {
                end += 1;
            }
            let chunk_text = text[start..end].to_string();
            let hash = format!("{:x}", md5::compute(chunk_text.as_bytes()));
            if seen_hashes.insert(hash) {
                chunks.push(FileChunk {
                    path: path_str.clone(),
                    text: chunk_text,
                    start_offset: start,
                });
            }

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

#[derive(Debug, Clone)]
pub struct FileScanResult {
    pub path: String,
    pub hash: String,
    pub chunks: Vec<FileChunk>,
}

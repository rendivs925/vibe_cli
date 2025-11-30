use memmap2::Mmap;
use rayon::prelude::*;
use shared::types::Result;
use shared::utils::is_supported_file;
use std::fs::File;
use std::path::{Path, PathBuf};

pub struct FileScanner {
    root_path: PathBuf,
}

impl FileScanner {
    pub fn new(root_path: impl Into<PathBuf>) -> Self {
        Self {
            root_path: root_path.into(),
        }
    }

    pub fn scan_files(&self) -> Result<Vec<FileChunk>> {
        let files = self.collect_files()?;
        let chunks: Vec<Result<Vec<FileChunk>>> = files
            .into_par_iter()
            .map(|path| self.load_and_chunk_file(&path))
            .collect();
        let mut all_chunks = Vec::new();
        for chunk_result in chunks {
            all_chunks.extend(chunk_result?);
        }
        Ok(all_chunks)
    }

    fn collect_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        self.collect_files_recursive(&self.root_path, &mut files)?;
        Ok(files)
    }

    fn collect_files_recursive(&self, dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                self.collect_files_recursive(&path, files)?;
            } else if is_supported_file(&path) {
                files.push(path);
            }
        }
        Ok(())
    }

    fn load_and_chunk_file(&self, path: &Path) -> Result<Vec<FileChunk>> {
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };
        let content = std::str::from_utf8(&mmap)?.to_string();
        Ok(self.chunk_text(content, path))
    }

    fn chunk_text(&self, text: String, path: &Path) -> Vec<FileChunk> {
        const CHUNK_SIZE: usize = 1000;
        const OVERLAP: usize = 200;

        let mut chunks = Vec::new();
        let mut start = 0;
        let path_str = path.to_string_lossy().to_string();

        while start < text.len() {
            let end = (start + CHUNK_SIZE).min(text.len());
            let chunk_text = text[start..end].to_string();
            chunks.push(FileChunk {
                path: path_str.clone(),
                text: chunk_text,
                start_offset: start,
            });

            if end == text.len() {
                break;
            }
            start = end.saturating_sub(OVERLAP);
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

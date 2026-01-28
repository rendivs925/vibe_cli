use serde::{Deserialize, Serialize};

/// Document entity representing a file or document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    id: String,
    path: String,
    content: String,
    content_type: DocumentType,
    size_bytes: u64,
    last_modified: chrono::DateTime<chrono::Utc>,
}

impl Document {
    pub fn new(
        id: String,
        path: String,
        content: String,
        content_type: DocumentType,
        size_bytes: u64,
        last_modified: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        Self {
            id,
            path,
            content,
            content_type,
            size_bytes,
            last_modified,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn content_type(&self) -> &DocumentType {
        &self.content_type
    }

    pub fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    pub fn last_modified(&self) -> chrono::DateTime<chrono::Utc> {
        self.last_modified
    }

    pub fn is_empty(&self) -> bool {
        self.content.trim().is_empty()
    }

    pub fn word_count(&self) -> usize {
        self.content.split_whitespace().count()
    }

    pub fn line_count(&self) -> usize {
        self.content.lines().count()
    }

    pub fn contains(&self, query: &str) -> bool {
        self.content.to_lowercase().contains(&query.to_lowercase())
    }

    pub fn excerpt(&self, max_chars: usize) -> String {
        if self.content.len() <= max_chars {
            self.content.clone()
        } else {
            format!("{}...", &self.content[..max_chars])
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DocumentType {
    PlainText,
    Markdown,
    Pdf,
    Docx,
    Code(CodeLanguage),
    Binary,
}

impl DocumentType {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "txt" => DocumentType::PlainText,
            "md" => DocumentType::Markdown,
            "pdf" => DocumentType::Pdf,
            "docx" => DocumentType::Docx,
            "rs" => DocumentType::Code(CodeLanguage::Rust),
            "js" => DocumentType::Code(CodeLanguage::JavaScript),
            "ts" => DocumentType::Code(CodeLanguage::TypeScript),
            "py" => DocumentType::Code(CodeLanguage::Python),
            "go" => DocumentType::Code(CodeLanguage::Go),
            "java" => DocumentType::Code(CodeLanguage::Java),
            "cpp" | "cxx" => DocumentType::Code(CodeLanguage::Cpp),
            "c" => DocumentType::Code(CodeLanguage::C),
            _ => DocumentType::PlainText,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            DocumentType::PlainText => "text/plain",
            DocumentType::Markdown => "text/markdown",
            DocumentType::Pdf => "application/pdf",
            DocumentType::Docx => {
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            }
            DocumentType::Code(lang) => lang.as_str(),
            DocumentType::Binary => "application/octet-stream",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CodeLanguage {
    Rust,
    JavaScript,
    TypeScript,
    Python,
    Go,
    Java,
    Cpp,
    C,
}

impl CodeLanguage {
    pub fn as_str(&self) -> &'static str {
        match self {
            CodeLanguage::Rust => "text/x-rust",
            CodeLanguage::JavaScript => "text/javascript",
            CodeLanguage::TypeScript => "text/typescript",
            CodeLanguage::Python => "text/x-python",
            CodeLanguage::Go => "text/x-go",
            CodeLanguage::Java => "text/x-java",
            CodeLanguage::Cpp => "text/x-c++",
            CodeLanguage::C => "text/x-c",
        }
    }
}

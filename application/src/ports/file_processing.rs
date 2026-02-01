use async_trait::async_trait;
use domain::entities::document::{Document, DocumentType};
use shared::error::AppError;

/// File processing port for handling different document types
#[async_trait]
pub trait DocumentReader: Send + Sync {
    /// Read and parse a document from a file path
    async fn read_document(&self, path: &str) -> Result<Document, AppError>;

    /// Check if this reader can handle the given file type
    fn can_handle(&self, file_path: &str) -> bool;

    /// Get supported file extensions
    fn supported_extensions(&self) -> Vec<&'static str>;

    /// Extract metadata from file
    async fn extract_metadata(&self, path: &str) -> Result<DocumentMetadata, AppError>;
}

/// Metadata for a document
#[derive(Debug, Clone)]
pub struct DocumentMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub created_date: Option<chrono::DateTime<chrono::Utc>>,
    pub modified_date: Option<chrono::DateTime<chrono::Utc>>,
    pub file_size: u64,
    pub page_count: Option<usize>,
    pub word_count: Option<usize>,
}

impl DocumentMetadata {
    pub fn new(file_size: u64) -> Self {
        Self {
            title: None,
            author: None,
            created_date: None,
            modified_date: None,
            file_size,
            page_count: None,
            word_count: None,
        }
    }

    pub fn with_title(mut self, title: String) -> Self {
        self.title = Some(title);
        self
    }

    pub fn with_author(mut self, author: String) -> Self {
        self.author = Some(author);
        self
    }

    pub fn with_created_date(mut self, date: chrono::DateTime<chrono::Utc>) -> Self {
        self.created_date = Some(date);
        self
    }

    pub fn with_modified_date(mut self, date: chrono::DateTime<chrono::Utc>) -> Self {
        self.modified_date = Some(date);
        self
    }

    pub fn with_page_count(mut self, count: usize) -> Self {
        self.page_count = Some(count);
        self
    }

    pub fn with_word_count(mut self, count: usize) -> Self {
        self.word_count = Some(count);
        self
    }
}

/// File scanner for finding documents
#[async_trait]
pub trait FileScanner: Send + Sync {
    /// Scan a directory for supported documents
    async fn scan_directory(&self, path: &str, recursive: bool) -> Result<Vec<String>, AppError>;

    /// Check if a path should be ignored
    fn should_ignore(&self, path: &str) -> bool;

    /// Get supported file patterns
    fn supported_patterns(&self) -> Vec<&'static str>;
}

/// Text extraction service
#[async_trait]
pub trait TextExtractor: Send + Sync {
    /// Extract plain text from a document
    async fn extract_text(&self, document_path: &str) -> Result<String, AppError>;

    /// Extract structured content from a document
    async fn extract_structured(&self, document_path: &str) -> Result<StructuredContent, AppError>;

    /// Check if this extractor can handle the file type
    fn can_handle(&self, file_path: &str) -> bool;
}

/// Structured content representation
#[derive(Debug, Clone)]
pub struct StructuredContent {
    pub title: Option<String>,
    pub headings: Vec<Heading>,
    pub paragraphs: Vec<Paragraph>,
    pub tables: Vec<Table>,
    pub images: Vec<Image>,
    pub code_blocks: Vec<CodeBlock>,
}

impl StructuredContent {
    pub fn new() -> Self {
        Self {
            title: None,
            headings: Vec::new(),
            paragraphs: Vec::new(),
            tables: Vec::new(),
            images: Vec::new(),
            code_blocks: Vec::new(),
        }
    }

    pub fn with_title(mut self, title: String) -> Self {
        self.title = Some(title);
        self
    }

    pub fn add_heading(mut self, heading: Heading) -> Self {
        self.headings.push(heading);
        self
    }

    pub fn add_paragraph(mut self, paragraph: Paragraph) -> Self {
        self.paragraphs.push(paragraph);
        self
    }

    pub fn add_table(mut self, table: Table) -> Self {
        self.tables.push(table);
        self
    }

    pub fn add_image(mut self, image: Image) -> Self {
        self.images.push(image);
        self
    }

    pub fn add_code_block(mut self, code_block: CodeBlock) -> Self {
        self.code_blocks.push(code_block);
        self
    }
}

#[derive(Debug, Clone)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub anchor: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Paragraph {
    pub text: String,
    pub style: ParagraphStyle,
}

#[derive(Debug, Clone)]
pub enum ParagraphStyle {
    Normal,
    Quote,
    List,
    Code,
}

#[derive(Debug, Clone)]
pub struct Table {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub caption: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Image {
    pub src: String,
    pub alt: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CodeBlock {
    pub language: Option<String>,
    pub code: String,
    pub line_numbers: bool,
}

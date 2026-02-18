use domain::entities::context_document::{ContextDocument, ContextDocumentType};

pub struct ContextVault {
    documents: Vec<ContextDocument>,
    ref_counter: u32,
}

impl ContextVault {
    pub fn new() -> Self {
        Self {
            documents: Vec::new(),
            ref_counter: 0,
        }
    }

    pub fn add(&mut self, doc_type: ContextDocumentType, label: &str, content: String) -> String {
        self.ref_counter += 1;
        let id = format!("REF-{:02}", self.ref_counter);
        let doc = ContextDocument::new(id.clone(), doc_type, label, content);
        self.documents.push(doc);
        id
    }

    pub fn add_with_source(
        &mut self,
        doc_type: ContextDocumentType,
        label: &str,
        content: String,
        source: &str,
    ) -> String {
        let id = self.add(doc_type, label, content);
        if let Some(doc) = self.documents.last_mut() {
            if !source.trim().is_empty() {
                doc.source_ref = Some(source.to_string());
            }
        }
        id
    }

    pub fn get(&self, id: &str) -> Option<&ContextDocument> {
        self.documents.iter().find(|d| d.id == id)
    }

    pub fn update(&mut self, id: &str, content: String) {
        if let Some(doc) = self.documents.iter_mut().find(|d| d.id == id) {
            doc.content = content;
            doc.timestamp = chrono::Utc::now();
        }
    }

    pub fn render(&self) -> String {
        self.documents
            .iter()
            .map(|d| d.to_markdown())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl Default for ContextVault {
    fn default() -> Self {
        Self::new()
    }
}

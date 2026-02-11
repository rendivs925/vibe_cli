use async_trait::async_trait;
use domain::repositories::symbolic_reasoning_repository::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// File-based implementation of symbolic reasoning repository
pub struct FileSymbolicStorage {
    base_path: PathBuf,
    format: StorageFormat,
}

impl FileSymbolicStorage {
    pub fn new<P: AsRef<Path>>(base_path: P, format: StorageFormat) -> Self {
        Self {
            base_path: base_path.as_ref().to_path_buf(),
            format,
        }
    }

    pub async fn initialize(&self) -> Result<(), SymbolicStorageError> {
        let dirs = ["traces", "expressions", "constraints"];
        for dir in &dirs {
            let path = self.base_path.join(dir);
            fs::create_dir_all(&path).await.map_err(|e| {
                SymbolicStorageError::StorageError(format!("Failed to create directory: {}", e))
            })?;
        }
        Ok(())
    }

    fn get_trace_path(&self, id: &str) -> PathBuf {
        self.base_path.join("traces").join(format!("{}.json", id))
    }

    fn get_expression_path(&self, id: &str) -> PathBuf {
        self.base_path
            .join("expressions")
            .join(format!("{}.json", id))
    }

    fn get_constraint_path(&self, id: &str) -> PathBuf {
        self.base_path
            .join("constraints")
            .join(format!("{}.json", id))
    }

    async fn serialize<T: Serialize>(&self, data: &T) -> Result<Vec<u8>, SymbolicStorageError> {
        match self.format {
            StorageFormat::Json => serde_json::to_vec(data)
                .map_err(|e| SymbolicStorageError::SerializationError(e.to_string())),
            StorageFormat::MessagePack => rmp_serde::to_vec(data)
                .map_err(|e| SymbolicStorageError::SerializationError(e.to_string())),
            StorageFormat::Cbor => serde_cbor::to_vec(data)
                .map_err(|e| SymbolicStorageError::SerializationError(e.to_string())),
            StorageFormat::Custom(ref format_name) => Err(SymbolicStorageError::FormatError(
                format!("Custom format '{}' not supported", format_name),
            )),
        }
    }

    async fn deserialize<T: for<'de> Deserialize<'de>>(
        &self,
        data: &[u8],
    ) -> Result<T, SymbolicStorageError> {
        match self.format {
            StorageFormat::Json => serde_json::from_slice(data)
                .map_err(|e| SymbolicStorageError::DeserializationError(e.to_string())),
            StorageFormat::MessagePack => rmp_serde::from_slice(data)
                .map_err(|e| SymbolicStorageError::DeserializationError(e.to_string())),
            StorageFormat::Cbor => serde_cbor::from_slice(data)
                .map_err(|e| SymbolicStorageError::DeserializationError(e.to_string())),
            StorageFormat::Custom(ref format_name) => Err(SymbolicStorageError::FormatError(
                format!("Custom format '{}' not supported", format_name),
            )),
        }
    }

    async fn read_file(&self, path: &Path) -> Result<Vec<u8>, SymbolicStorageError> {
        let metadata = fs::metadata(path)
            .await
            .map_err(|e| SymbolicStorageError::NotFound(format!("File not found: {}", e)))?;

        if metadata.len() > 1024 * 1024 {
            let file = fs::File::open(path)
                .await
                .map_err(|e| SymbolicStorageError::NotFound(format!("File not found: {}", e)))?;
            let mmap = unsafe {
                memmap2::Mmap::map(&file).map_err(|e| {
                    SymbolicStorageError::StorageError(format!("Failed to mmap file: {}", e))
                })?
            };
            Ok(mmap.to_vec())
        } else {
            let mut file = fs::File::open(path)
                .await
                .map_err(|e| SymbolicStorageError::NotFound(format!("File not found: {}", e)))?;
            let mut contents = Vec::new();
            file.read_to_end(&mut contents).await.map_err(|e| {
                SymbolicStorageError::StorageError(format!("Failed to read file: {}", e))
            })?;
            Ok(contents)
        }
    }

    async fn write_file(&self, path: &Path, data: &[u8]) -> Result<(), SymbolicStorageError> {
        let mut file = fs::File::create(path).await.map_err(|e| {
            SymbolicStorageError::StorageError(format!("Failed to create file: {}", e))
        })?;
        file.write_all(data).await.map_err(|e| {
            SymbolicStorageError::StorageError(format!("Failed to write file: {}", e))
        })?;
        Ok(())
    }

    fn generate_id(&self, content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content);
        hasher.update(chrono::Utc::now().to_rfc3339());
        format!("{:x}", hasher.finalize())[..16].to_string()
    }
}

#[async_trait]
impl SymbolicReasoningRepository for FileSymbolicStorage {
    async fn save_trace(
        &self,
        trace: &SymbolicReasoningTrace,
    ) -> Result<String, SymbolicStorageError> {
        let id = if trace.id.is_empty() {
            self.generate_id(&serde_json::to_string(trace).unwrap_or_default())
        } else {
            trace.id.clone()
        };

        let path = self.get_trace_path(&id);
        let data = self.serialize(trace).await?;
        self.write_file(&path, &data).await?;

        Ok(id)
    }

    async fn find_trace_by_id(
        &self,
        id: &str,
    ) -> Result<Option<SymbolicReasoningTrace>, SymbolicStorageError> {
        let path = self.get_trace_path(id);
        if !path.exists() {
            return Ok(None);
        }

        let data = self.read_file(&path).await?;
        let trace: SymbolicReasoningTrace = self.deserialize(&data).await?;
        Ok(Some(trace))
    }

    async fn save_expression(
        &self,
        expression: &SymbolicExpressionData,
    ) -> Result<String, SymbolicStorageError> {
        let id = if expression.id.is_empty() {
            self.generate_id(&serde_json::to_string(expression).unwrap_or_default())
        } else {
            expression.id.clone()
        };

        let path = self.get_expression_path(&id);
        let data = self.serialize(expression).await?;
        self.write_file(&path, &data).await?;

        Ok(id)
    }

    async fn find_expression_by_id(
        &self,
        id: &str,
    ) -> Result<Option<SymbolicExpressionData>, SymbolicStorageError> {
        let path = self.get_expression_path(id);
        if !path.exists() {
            return Ok(None);
        }

        let data = self.read_file(&path).await?;
        let expression: SymbolicExpressionData = self.deserialize(&data).await?;
        Ok(Some(expression))
    }

    async fn save_constraints(
        &self,
        constraints: &ConstraintSet,
    ) -> Result<String, SymbolicStorageError> {
        let id = if constraints.id.is_empty() {
            self.generate_id(&serde_json::to_string(constraints).unwrap_or_default())
        } else {
            constraints.id.clone()
        };

        let path = self.get_constraint_path(&id);
        let data = self.serialize(constraints).await?;
        self.write_file(&path, &data).await?;

        Ok(id)
    }

    async fn find_constraints_by_id(
        &self,
        id: &str,
    ) -> Result<Option<ConstraintSet>, SymbolicStorageError> {
        let path = self.get_constraint_path(id);
        if !path.exists() {
            return Ok(None);
        }

        let data = self.read_file(&path).await?;
        let constraints: ConstraintSet = self.deserialize(&data).await?;
        Ok(Some(constraints))
    }

    async fn query_traces(
        &self,
        query: &SymbolicQuery,
    ) -> Result<Vec<SymbolicReasoningTrace>, SymbolicStorageError> {
        let traces_dir = self.base_path.join("traces");
        let mut entries = fs::read_dir(&traces_dir).await.map_err(|e| {
            SymbolicStorageError::StorageError(format!("Failed to read traces directory: {}", e))
        })?;

        let mut results = Vec::new();

        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            SymbolicStorageError::StorageError(format!("Failed to read directory entry: {}", e))
        })? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            let data = match self.read_file(&path).await {
                Ok(d) => d,
                Err(_) => continue,
            };

            let trace: SymbolicReasoningTrace = match self.deserialize(&data).await {
                Ok(t) => t,
                Err(_) => continue,
            };

            // Apply filters
            if let Some(ref domain) = query.domain {
                if trace.domain != *domain {
                    continue;
                }
            }

            if let Some(ref from) = query.from_timestamp {
                if trace.timestamp < *from {
                    continue;
                }
            }

            if let Some(ref to) = query.to_timestamp {
                if trace.timestamp > *to {
                    continue;
                }
            }

            // Check metadata filter
            let metadata_matches = query
                .metadata_filter
                .iter()
                .all(|(key, value)| trace.metadata.get(key).map(|v| v == value).unwrap_or(false));
            if !metadata_matches {
                continue;
            }

            results.push(trace);
        }

        // Sort by timestamp descending
        results.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        // Apply offset and limit
        let offset = query.offset.unwrap_or(0);
        let limit = query.limit.unwrap_or(results.len());
        let end = (offset + limit).min(results.len());

        if offset < results.len() {
            Ok(results[offset..end].to_vec())
        } else {
            Ok(Vec::new())
        }
    }

    async fn delete_trace(&self, id: &str) -> Result<(), SymbolicStorageError> {
        let path = self.get_trace_path(id);
        if path.exists() {
            fs::remove_file(&path).await.map_err(|e| {
                SymbolicStorageError::StorageError(format!("Failed to delete trace: {}", e))
            })?;
        }
        Ok(())
    }

    async fn get_stats(&self) -> Result<SymbolicStorageStats, SymbolicStorageError> {
        let traces_dir = self.base_path.join("traces");
        let expressions_dir = self.base_path.join("expressions");
        let constraints_dir = self.base_path.join("constraints");

        let mut total_traces = 0usize;
        let mut total_size: u64 = 0;
        let mut oldest: Option<chrono::DateTime<chrono::Utc>> = None;
        let mut newest: Option<chrono::DateTime<chrono::Utc>> = None;

        if let Ok(mut entries) = fs::read_dir(&traces_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    total_traces += 1;
                    if let Ok(metadata) = entry.metadata().await {
                        total_size += metadata.len();
                    }

                    if let Ok(data) = self.read_file(&path).await {
                        if let Ok(trace) = self.deserialize::<SymbolicReasoningTrace>(&data).await {
                            oldest = oldest
                                .map(|o| o.min(trace.timestamp))
                                .or(Some(trace.timestamp));
                            newest = newest
                                .map(|n| n.max(trace.timestamp))
                                .or(Some(trace.timestamp));
                        }
                    }
                }
            }
        }

        let mut total_expressions = 0usize;
        if let Ok(mut entries) = fs::read_dir(&expressions_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if entry.path().extension().and_then(|s| s.to_str()) == Some("json") {
                    total_expressions += 1;
                }
            }
        }

        let mut total_constraints = 0usize;
        if let Ok(mut entries) = fs::read_dir(&constraints_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if entry.path().extension().and_then(|s| s.to_str()) == Some("json") {
                    total_constraints += 1;
                }
            }
        }

        let avg_size = if total_traces > 0 {
            total_size / total_traces as u64
        } else {
            0
        };

        Ok(SymbolicStorageStats::new(
            total_traces,
            total_expressions,
            total_constraints,
            total_size,
            oldest,
            newest,
            avg_size,
        ))
    }

    async fn list_trace_ids(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<String>, SymbolicStorageError> {
        let traces_dir = self.base_path.join("traces");
        let mut entries = fs::read_dir(&traces_dir).await.map_err(|e| {
            SymbolicStorageError::StorageError(format!("Failed to read traces directory: {}", e))
        })?;

        let mut ids = Vec::new();

        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            SymbolicStorageError::StorageError(format!("Failed to read directory entry: {}", e))
        })? {
            let path = entry.path();
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                ids.push(stem.to_string());
            }
        }

        ids.sort();
        let end = (offset + limit).min(ids.len());

        if offset < ids.len() {
            Ok(ids[offset..end].to_vec())
        } else {
            Ok(Vec::new())
        }
    }
}

/// Builder for creating FileSymbolicStorage instances
pub struct FileSymbolicStorageBuilder {
    base_path: PathBuf,
    format: StorageFormat,
}

impl FileSymbolicStorageBuilder {
    pub fn new<P: AsRef<Path>>(base_path: P) -> Self {
        Self {
            base_path: base_path.as_ref().to_path_buf(),
            format: StorageFormat::Json,
        }
    }

    pub fn with_format(mut self, format: StorageFormat) -> Self {
        self.format = format;
        self
    }

    pub async fn build(self) -> Result<FileSymbolicStorage, SymbolicStorageError> {
        let storage = FileSymbolicStorage::new(self.base_path, self.format);
        storage.initialize().await?;
        Ok(storage)
    }
}

/// In-memory storage implementation for testing
pub struct InMemorySymbolicStorage {
    traces: std::sync::Mutex<HashMap<String, SymbolicReasoningTrace>>,
    expressions: std::sync::Mutex<HashMap<String, SymbolicExpressionData>>,
    constraints: std::sync::Mutex<HashMap<String, ConstraintSet>>,
}

impl InMemorySymbolicStorage {
    pub fn new() -> Self {
        Self {
            traces: std::sync::Mutex::new(HashMap::new()),
            expressions: std::sync::Mutex::new(HashMap::new()),
            constraints: std::sync::Mutex::new(HashMap::new()),
        }
    }

    fn generate_id(&self) -> String {
        format!("mem_{}", uuid::Uuid::new_v4().to_string()[..8].to_string())
    }
}

#[async_trait]
impl SymbolicReasoningRepository for InMemorySymbolicStorage {
    async fn save_trace(
        &self,
        trace: &SymbolicReasoningTrace,
    ) -> Result<String, SymbolicStorageError> {
        let id = if trace.id.is_empty() {
            self.generate_id()
        } else {
            trace.id.clone()
        };

        let mut traces = self.traces.lock().map_err(|_| {
            SymbolicStorageError::StorageError("Failed to lock traces mutex".to_string())
        })?;

        traces.insert(id.clone(), trace.clone());
        Ok(id)
    }

    async fn find_trace_by_id(
        &self,
        id: &str,
    ) -> Result<Option<SymbolicReasoningTrace>, SymbolicStorageError> {
        let traces = self.traces.lock().map_err(|_| {
            SymbolicStorageError::StorageError("Failed to lock traces mutex".to_string())
        })?;

        Ok(traces.get(id).cloned())
    }

    async fn save_expression(
        &self,
        expression: &SymbolicExpressionData,
    ) -> Result<String, SymbolicStorageError> {
        let id = if expression.id.is_empty() {
            self.generate_id()
        } else {
            expression.id.clone()
        };

        let mut expressions = self.expressions.lock().map_err(|_| {
            SymbolicStorageError::StorageError("Failed to lock expressions mutex".to_string())
        })?;

        expressions.insert(id.clone(), expression.clone());
        Ok(id)
    }

    async fn find_expression_by_id(
        &self,
        id: &str,
    ) -> Result<Option<SymbolicExpressionData>, SymbolicStorageError> {
        let expressions = self.expressions.lock().map_err(|_| {
            SymbolicStorageError::StorageError("Failed to lock expressions mutex".to_string())
        })?;

        Ok(expressions.get(id).cloned())
    }

    async fn save_constraints(
        &self,
        constraints: &ConstraintSet,
    ) -> Result<String, SymbolicStorageError> {
        let id = if constraints.id.is_empty() {
            self.generate_id()
        } else {
            constraints.id.clone()
        };

        let mut constraint_map = self.constraints.lock().map_err(|_| {
            SymbolicStorageError::StorageError("Failed to lock constraints mutex".to_string())
        })?;

        constraint_map.insert(id.clone(), constraints.clone());
        Ok(id)
    }

    async fn find_constraints_by_id(
        &self,
        id: &str,
    ) -> Result<Option<ConstraintSet>, SymbolicStorageError> {
        let constraints = self.constraints.lock().map_err(|_| {
            SymbolicStorageError::StorageError("Failed to lock constraints mutex".to_string())
        })?;

        Ok(constraints.get(id).cloned())
    }

    async fn query_traces(
        &self,
        query: &SymbolicQuery,
    ) -> Result<Vec<SymbolicReasoningTrace>, SymbolicStorageError> {
        let traces = self.traces.lock().map_err(|_| {
            SymbolicStorageError::StorageError("Failed to lock traces mutex".to_string())
        })?;

        let mut results: Vec<_> = traces
            .values()
            .filter(|trace| {
                if let Some(ref domain) = query.domain {
                    if trace.domain != *domain {
                        return false;
                    }
                }

                if let Some(ref from) = query.from_timestamp {
                    if trace.timestamp < *from {
                        return false;
                    }
                }

                if let Some(ref to) = query.to_timestamp {
                    if trace.timestamp > *to {
                        return false;
                    }
                }

                query.metadata_filter.iter().all(|(key, value)| {
                    trace.metadata.get(key).map(|v| v == value).unwrap_or(false)
                })
            })
            .cloned()
            .collect();

        results.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        let offset = query.offset.unwrap_or(0);
        let limit = query.limit.unwrap_or(results.len());
        let end = (offset + limit).min(results.len());

        if offset < results.len() {
            Ok(results[offset..end].to_vec())
        } else {
            Ok(Vec::new())
        }
    }

    async fn delete_trace(&self, id: &str) -> Result<(), SymbolicStorageError> {
        let mut traces = self.traces.lock().map_err(|_| {
            SymbolicStorageError::StorageError("Failed to lock traces mutex".to_string())
        })?;

        traces.remove(id);
        Ok(())
    }

    async fn get_stats(&self) -> Result<SymbolicStorageStats, SymbolicStorageError> {
        let traces = self.traces.lock().map_err(|_| {
            SymbolicStorageError::StorageError("Failed to lock traces mutex".to_string())
        })?;

        let expressions = self.expressions.lock().map_err(|_| {
            SymbolicStorageError::StorageError("Failed to lock expressions mutex".to_string())
        })?;

        let constraints = self.constraints.lock().map_err(|_| {
            SymbolicStorageError::StorageError("Failed to lock constraints mutex".to_string())
        })?;

        let mut oldest: Option<chrono::DateTime<chrono::Utc>> = None;
        let mut newest: Option<chrono::DateTime<chrono::Utc>> = None;

        for trace in traces.values() {
            oldest = oldest
                .map(|o| o.min(trace.timestamp))
                .or(Some(trace.timestamp));
            newest = newest
                .map(|n| n.max(trace.timestamp))
                .or(Some(trace.timestamp));
        }

        Ok(SymbolicStorageStats::new(
            traces.len(),
            expressions.len(),
            constraints.len(),
            0, // No size tracking for in-memory
            oldest,
            newest,
            0,
        ))
    }

    async fn list_trace_ids(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<String>, SymbolicStorageError> {
        let traces = self.traces.lock().map_err(|_| {
            SymbolicStorageError::StorageError("Failed to lock traces mutex".to_string())
        })?;

        let mut ids: Vec<_> = traces.keys().cloned().collect();
        ids.sort();

        let end = (offset + limit).min(ids.len());

        if offset < ids.len() {
            Ok(ids[offset..end].to_vec())
        } else {
            Ok(Vec::new())
        }
    }
}

impl Default for InMemorySymbolicStorage {
    fn default() -> Self {
        Self::new()
    }
}

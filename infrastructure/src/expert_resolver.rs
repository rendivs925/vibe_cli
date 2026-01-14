use serde::{Deserialize, Serialize};
use shared::types::Result;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use tokio::sync::RwLock;

/// Expert dependency resolution system
pub struct ExpertResolver {
    experts: RwLock<HashMap<String, Expert>>,
    knowledge_base_path: PathBuf,
    dependency_cache: RwLock<HashMap<String, Vec<String>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Expert {
    pub name: String,
    pub domain: String,
    pub priority: u32,
    pub dependencies: Vec<String>,
    pub capabilities: Vec<String>,
    pub metadata: HashMap<String, String>,
    pub path: PathBuf,
    pub loaded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpertMetadata {
    pub name: String,
    pub version: String,
    pub domain: String,
    pub priority: u32,
    pub dependencies: Vec<String>,
    pub capabilities: Vec<String>,
    pub description: String,
    pub tags: Vec<String>,
    pub author: String,
    pub created_at: String,
}

#[derive(Debug)]
pub struct ResolutionResult {
    pub experts: Vec<Expert>,
    pub dependency_chain: Vec<String>,
    pub missing_dependencies: Vec<String>,
    pub conflicts: Vec<String>,
}

impl ExpertResolver {
    /// Create new expert resolver
    pub async fn new(knowledge_base_path: PathBuf) -> Result<Self> {
        let resolver = Self {
            experts: RwLock::new(HashMap::new()),
            knowledge_base_path,
            dependency_cache: RwLock::new(HashMap::new()),
        };

        resolver.load_all_experts().await?;
        Ok(resolver)
    }

    /// Load all available experts from knowledge base
    async fn load_all_experts(&self) -> Result<()> {
        let knowledge_path = &self.knowledge_base_path;

        if !knowledge_path.exists() {
            fs::create_dir_all(knowledge_path)?;
            return Ok(()); // No experts yet
        }

        let mut experts = self.experts.write().await;

        // Scan for expert directories
        for entry in fs::read_dir(knowledge_path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                if let Some(expert) = self.load_expert_from_path(&path).await? {
                    experts.insert(expert.name.clone(), expert);
                }
            }
        }

        Ok(())
    }

    /// Load expert from directory path
    async fn load_expert_from_path(&self, path: &Path) -> Result<Option<Expert>> {
        let metadata_path = path.join("metadata.json");

        if !metadata_path.exists() {
            return Ok(None);
        }

        let metadata_content = fs::read_to_string(&metadata_path)?;
        let metadata: ExpertMetadata = serde_json::from_str(&metadata_content)?;

        let expert = Expert {
            name: metadata.name.clone(),
            domain: metadata.domain,
            priority: metadata.priority,
            dependencies: metadata.dependencies,
            capabilities: metadata.capabilities,
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("version".to_string(), metadata.version);
                meta.insert("description".to_string(), metadata.description);
                meta.insert("author".to_string(), metadata.author);
                meta.insert("created_at".to_string(), metadata.created_at);
                for tag in metadata.tags {
                    meta.insert(format!("tag_{}", tag), "true".to_string());
                }
                meta
            },
            path: path.to_path_buf(),
            loaded: false,
        };

        Ok(Some(expert))
    }

    /// Resolve experts for a given query with dependency resolution
    pub async fn resolve_experts(
        &self,
        query: &str,
        required_capabilities: &[String],
    ) -> Result<ResolutionResult> {
        let experts = self.experts.read().await;

        // Find candidate experts based on capabilities and query matching
        let mut candidates = self
            .find_candidates(&experts, query, required_capabilities)
            .await;

        // Sort by priority (higher priority first)
        candidates.sort_by(|a, b| b.priority.cmp(&a.priority));

        // Resolve dependencies
        let (resolved, missing, conflicts) = self.resolve_dependencies(&candidates).await;

        let dependency_chain = self.build_dependency_chain(&resolved);

        Ok(ResolutionResult {
            experts: resolved,
            dependency_chain,
            missing_dependencies: missing,
            conflicts,
        })
    }

    /// Find candidate experts based on query and capabilities
    async fn find_candidates(
        &self,
        experts: &HashMap<String, Expert>,
        query: &str,
        required_capabilities: &[String],
    ) -> Vec<Expert> {
        let mut candidates = Vec::new();
        let query_lower = query.to_lowercase();

        for expert in experts.values() {
            let mut score = 0;

            // Check capabilities match
            for req_cap in required_capabilities {
                if expert.capabilities.contains(req_cap) {
                    score += 10;
                }
            }

            // Check domain relevance
            if query_lower.contains(&expert.domain.to_lowercase()) {
                score += 5;
            }

            // Check metadata tags
            for (key, _value) in &expert.metadata {
                if key.starts_with("tag_") && query_lower.contains(&key[4..].to_lowercase()) {
                    score += 3;
                }
            }

            // Check description relevance
            if let Some(desc) = expert.metadata.get("description") {
                if query_lower
                    .split_whitespace()
                    .any(|word| desc.to_lowercase().contains(word))
                {
                    score += 2;
                }
            }

            if score > 0 {
                let mut candidate = expert.clone();
                candidate
                    .metadata
                    .insert("relevance_score".to_string(), score.to_string());
                candidates.push(candidate);
            }
        }

        candidates
    }

    /// Resolve dependencies with cycle detection and conflict resolution
    async fn resolve_dependencies(
        &self,
        candidates: &[Expert],
    ) -> (Vec<Expert>, Vec<String>, Vec<String>) {
        let mut resolved = Vec::new();
        let mut missing = Vec::new();
        let mut conflicts = Vec::new();
        let mut visited = HashSet::new();

        let experts = self.experts.read().await;

        // Iterative dependency resolution using topological sort
        for candidate in candidates {
            if !visited.contains(&candidate.name) {
                self.resolve_expert_iterative(
                    &experts,
                    candidate,
                    &mut resolved,
                    &mut visited,
                    &mut missing,
                    &mut conflicts,
                );
            }
        }

        (resolved, missing, conflicts)
    }

    /// Iteratively resolve expert dependencies to avoid recursion
    fn resolve_expert_iterative(
        &self,
        experts: &HashMap<String, Expert>,
        expert: &Expert,
        resolved: &mut Vec<Expert>,
        visited: &mut HashSet<String>,
        missing: &mut Vec<String>,
        conflicts: &mut Vec<String>,
    ) {
        // Simple iterative approach - check dependencies
        for dep_name in &expert.dependencies {
            if let Some(dep_expert) = experts.get(dep_name) {
                if !visited.contains(dep_name) {
                    self.resolve_expert_iterative(
                        experts, dep_expert, resolved, visited, missing, conflicts,
                    );
                }
            } else {
                if !missing.contains(dep_name) {
                    missing.push(dep_name.clone());
                }
            }
        }

        if !visited.contains(&expert.name) {
            visited.insert(expert.name.clone());
            resolved.push(expert.clone());
        }
    }

    /// Build dependency chain for resolved experts
    fn build_dependency_chain(&self, experts: &[Expert]) -> Vec<String> {
        let mut chain = Vec::new();
        let mut added = HashSet::new();

        // Topological sort for dependency chain
        for expert in experts {
            if !added.contains(&expert.name) {
                self.add_to_chain(expert, experts, &mut chain, &mut added);
            }
        }

        chain
    }

    /// Add expert to dependency chain recursively
    fn add_to_chain(
        &self,
        expert: &Expert,
        all_experts: &[Expert],
        chain: &mut Vec<String>,
        added: &mut HashSet<String>,
    ) {
        // Add dependencies first
        for dep_name in &expert.dependencies {
            if !added.contains(dep_name) {
                // Find dependency in resolved list
                if let Some(dep) = all_experts.iter().find(|e| e.name == *dep_name) {
                    self.add_to_chain(dep, all_experts, chain, added);
                }
            }
        }

        // Add this expert
        if !added.contains(&expert.name) {
            chain.push(expert.name.clone());
            added.insert(expert.name.clone());
        }
    }

    /// Load resolved experts into memory
    pub async fn load_experts(&self, experts: &[Expert]) -> Result<()> {
        for expert in experts {
            if !expert.loaded {
                self.load_expert_data(expert).await?;
            }
        }
        Ok(())
    }

    /// Load expert data (embeddings, knowledge, etc.)
    async fn load_expert_data(&self, expert: &Expert) -> Result<()> {
        // For now, just mark as loaded
        // In a full implementation, this would load embeddings, models, etc.
        let mut experts = self.experts.write().await;
        if let Some(e) = experts.get_mut(&expert.name) {
            e.loaded = true;
        }

        Ok(())
    }

    /// Get expert statistics
    pub async fn get_stats(&self) -> HashMap<String, String> {
        let experts = self.experts.read().await;
        let mut stats = HashMap::new();

        stats.insert("total_experts".to_string(), experts.len().to_string());

        let loaded_count = experts.values().filter(|e| e.loaded).count();
        stats.insert("loaded_experts".to_string(), loaded_count.to_string());

        let domains: HashSet<String> = experts.values().map(|e| e.domain.clone()).collect();
        stats.insert("unique_domains".to_string(), domains.len().to_string());

        let total_deps: usize = experts.values().map(|e| e.dependencies.len()).sum();
        stats.insert("total_dependencies".to_string(), total_deps.to_string());

        stats
    }

    /// Create new expert with metadata
    pub async fn create_expert(
        &self,
        metadata: ExpertMetadata,
        path: Option<PathBuf>,
    ) -> Result<()> {
        let expert_path = path.unwrap_or_else(|| self.knowledge_base_path.join(&metadata.name));

        fs::create_dir_all(&expert_path)?;

        let metadata_path = expert_path.join("metadata.json");
        let metadata_json = serde_json::to_string_pretty(&metadata)?;
        fs::write(metadata_path, metadata_json)?;

        // Reload experts to include the new one
        self.load_all_experts().await?;

        Ok(())
    }

    /// Remove expert
    pub async fn remove_expert(&self, name: &str) -> Result<()> {
        let expert_path = self.knowledge_base_path.join(name);

        if expert_path.exists() {
            fs::remove_dir_all(expert_path)?;
        }

        let mut experts = self.experts.write().await;
        experts.remove(name);

        Ok(())
    }

    /// Update expert metadata
    pub async fn update_expert_metadata(&self, name: &str, metadata: ExpertMetadata) -> Result<()> {
        let expert_path = self.knowledge_base_path.join(name);
        let metadata_path = expert_path.join("metadata.json");

        let metadata_json = serde_json::to_string_pretty(&metadata)?;
        fs::write(metadata_path, metadata_json)?;

        // Reload the expert
        if let Some(updated) = self.load_expert_from_path(&expert_path).await? {
            let mut experts = self.experts.write().await;
            experts.insert(name.to_string(), updated);
        }

        Ok(())
    }

    /// Get experts by domain
    pub async fn get_experts_by_domain(&self, domain: &str) -> Vec<Expert> {
        let experts = self.experts.read().await;
        experts
            .values()
            .filter(|e| e.domain == domain)
            .cloned()
            .collect()
    }

    /// Get expert capabilities summary
    pub async fn get_capabilities_summary(&self) -> HashMap<String, Vec<String>> {
        let experts = self.experts.read().await;
        let mut summary = HashMap::new();

        for expert in experts.values() {
            for capability in &expert.capabilities {
                summary
                    .entry(capability.clone())
                    .or_insert_with(Vec::new)
                    .push(expert.name.clone());
            }
        }

        summary
    }

    /// Validate expert dependencies
    pub async fn validate_dependencies(&self) -> Vec<String> {
        let experts = self.experts.read().await;
        let mut issues = Vec::new();

        for expert in experts.values() {
            for dep in &expert.dependencies {
                if !experts.contains_key(dep) {
                    issues.push(format!(
                        "Expert '{}' depends on missing expert '{}'",
                        expert.name, dep
                    ));
                }
            }
        }

        issues
    }
}

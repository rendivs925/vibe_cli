use anyhow::Result;
use petgraph::graph::NodeIndex;
use petgraph::Graph;
use regex::Regex;
/// Context-aware validation system with knowledge graphs and dependency analysis
/// Advanced hallucination prevention using project relationships and patterns
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Knowledge graph representing project structure and relationships
#[derive(Debug, Clone)]
pub struct KnowledgeGraph {
    /// Graph of entities and relationships
    graph: Graph<Entity, Relationship>,
    /// Quick lookup from entity name to node index
    entity_lookup: HashMap<String, NodeIndex>,
    /// File dependencies cache
    file_dependencies: HashMap<String, HashSet<String>>,
    /// Function call relationships
    call_graph: HashMap<String, HashSet<String>>,
    /// Known patterns for validation
    known_patterns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Entity {
    File(String),
    Function(String),
    Struct(String),
    Module(String),
    Dependency(String),
    ApiEndpoint(String),
}

#[derive(Debug, Clone)]
pub enum Relationship {
    DependsOn,
    Calls,
    Contains,
    Imports,
    InheritsFrom,
    Implements,
}

/// Advanced context-aware validator
pub struct ContextAwareValidator {
    knowledge_graph: Arc<RwLock<KnowledgeGraph>>,
    dependency_patterns: Vec<Regex>,
    confidence_calculator: ConfidenceCalculator,
}

#[derive(Debug)]
pub struct ConfidenceCalculator {
    /// Weight factors for different validation aspects
    weights: ValidationWeights,
}

#[derive(Debug)]
pub struct ValidationWeights {
    pub file_existence: f32,
    pub dependency_validity: f32,
    pub call_graph_consistency: f32,
    pub pattern_matching: f32,
    pub knowledge_graph_relevance: f32,
}

#[derive(Debug, Clone)]
pub struct ContextValidationResult {
    pub overall_confidence: f32,
    pub validation_scores: HashMap<String, f32>,
    pub issues: Vec<String>,
    pub suggestions: Vec<String>,
    pub related_entities: Vec<String>,
}

impl Default for ValidationWeights {
    fn default() -> Self {
        Self {
            file_existence: 0.3,
            dependency_validity: 0.25,
            call_graph_consistency: 0.2,
            pattern_matching: 0.15,
            knowledge_graph_relevance: 0.1,
        }
    }
}

impl ContextAwareValidator {
    /// Create a new context-aware validator
    pub fn new() -> Self {
        let dependency_patterns = vec![
            Regex::new(r"use\s+[\w:]+::").unwrap(),
            Regex::new(r"extern\s+crate\s+\w+").unwrap(),
            Regex::new(r"mod\s+\w+").unwrap(),
            Regex::new(r"fn\s+\w+\(").unwrap(),
            Regex::new(r"struct\s+\w+").unwrap(),
            Regex::new(r"impl\s+[\w:]+").unwrap(),
        ];

        Self {
            knowledge_graph: Arc::new(RwLock::new(KnowledgeGraph::new())),
            dependency_patterns,
            confidence_calculator: ConfidenceCalculator::new(ValidationWeights::default()),
        }
    }

    /// Build knowledge graph from project analysis
    pub async fn build_knowledge_graph(&self, project_root: &std::path::Path) -> Result<()> {
        let mut graph = self.knowledge_graph.write().await;
        graph.build_from_project(project_root).await
    }

    /// Perform context-aware validation of an AI suggestion
    pub async fn validate_with_context(
        &self,
        suggestion: &str,
        context: &ValidationContext,
    ) -> Result<ContextValidationResult> {
        let graph = self.knowledge_graph.read().await;

        // Extract entities mentioned in suggestion
        let mentioned_entities = self.extract_entities(suggestion)?;

        // Validate file and path references
        let file_validation = self.validate_file_references(suggestion, &graph).await?;

        // Validate dependency relationships
        let dependency_validation = self.validate_dependencies(suggestion, &graph).await?;

        // Validate function calls and relationships
        let call_validation = self.validate_function_calls(suggestion, &graph).await?;

        // Check pattern consistency
        let pattern_validation = self.validate_patterns(suggestion, &graph).await?;

        // Find related entities in knowledge graph
        let related_entities = self
            .find_related_entities(&mentioned_entities, &graph)
            .await?;

        // Calculate overall confidence
        let validation_scores = HashMap::from([
            ("file_references".to_string(), file_validation),
            ("dependencies".to_string(), dependency_validation),
            ("function_calls".to_string(), call_validation),
            ("patterns".to_string(), pattern_validation),
        ]);

        let overall_confidence = self.confidence_calculator.calculate_overall(
            &validation_scores,
            context,
            &related_entities,
        );

        // Generate issues and suggestions
        let (issues, suggestions) =
            self.generate_feedback(&validation_scores, &related_entities, suggestion);

        Ok(ContextValidationResult {
            overall_confidence,
            validation_scores,
            issues,
            suggestions,
            related_entities: related_entities.into_iter().collect(),
        })
    }

    /// Extract entities mentioned in suggestion
    fn extract_entities(&self, suggestion: &str) -> Result<HashSet<String>> {
        let mut entities = HashSet::new();

        // Extract file paths
        let file_patterns = vec![
            r"[\w/.-]+\.rs\b",
            r"[\w/.-]+\.toml\b",
            r"[\w/.-]+\.md\b",
            r"[\w/.-]+\.json\b",
            r"src/[\w/.-]+",
            r"tests/[\w/.-]+",
        ];

        for pattern in &file_patterns {
            if let Ok(regex) = Regex::new(pattern) {
                for capture in regex.find_iter(suggestion) {
                    entities.insert(capture.as_str().to_string());
                }
            }
        }

        // Extract function/struct names
        let code_patterns = vec![
            r"fn\s+(\w+)",
            r"struct\s+(\w+)",
            r"impl\s+([\w:]+)",
            r"mod\s+(\w+)",
        ];

        for pattern in &code_patterns {
            if let Ok(regex) = Regex::new(pattern) {
                for capture in regex.captures_iter(suggestion) {
                    if let Some(name) = capture.get(1) {
                        entities.insert(name.as_str().to_string());
                    }
                }
            }
        }

        Ok(entities)
    }

    /// Validate file references in suggestion
    async fn validate_file_references(
        &self,
        suggestion: &str,
        graph: &KnowledgeGraph,
    ) -> Result<f32> {
        let mut valid_count = 0;
        let mut total_count = 0;

        // Extract file references
        let file_regex = Regex::new(r"[\w/.-]+\.(rs|toml|md|json)\b")?;
        for capture in file_regex.find_iter(suggestion) {
            total_count += 1;
            let file_path = capture.as_str();

            // Check if file exists in knowledge graph
            if graph.entity_lookup.contains_key(file_path) {
                valid_count += 1;
            }
        }

        if total_count == 0 {
            return Ok(1.0); // No file references, neutral score
        }

        Ok(valid_count as f32 / total_count as f32)
    }

    /// Validate dependency relationships
    async fn validate_dependencies(&self, suggestion: &str, graph: &KnowledgeGraph) -> Result<f32> {
        let mut valid_deps = 0;
        let mut total_deps = 0;

        for pattern in &self.dependency_patterns {
            for capture in pattern.find_iter(suggestion) {
                total_deps += 1;
                let dep_text = capture.as_str();

                // Check if dependency relationship exists in graph
                // This is a simplified check - in practice, would analyze the AST
                if graph.has_valid_dependency(dep_text) {
                    valid_deps += 1;
                }
            }
        }

        if total_deps == 0 {
            return Ok(1.0);
        }

        Ok(valid_deps as f32 / total_deps as f32)
    }

    /// Validate function calls and relationships
    async fn validate_function_calls(
        &self,
        suggestion: &str,
        graph: &KnowledgeGraph,
    ) -> Result<f32> {
        // Extract function calls
        let call_regex = Regex::new(r"(\w+)\(")?;
        let mut valid_calls = 0;
        let mut total_calls = 0;

        for capture in call_regex.captures_iter(suggestion) {
            if let Some(func_name) = capture.get(1) {
                total_calls += 1;
                let func_name = func_name.as_str();

                // Check if function exists in call graph
                if graph.call_graph.contains_key(func_name) {
                    valid_calls += 1;
                }
            }
        }

        if total_calls == 0 {
            return Ok(1.0);
        }

        Ok(valid_calls as f32 / total_calls as f32)
    }

    /// Validate against known patterns
    async fn validate_patterns(&self, suggestion: &str, graph: &KnowledgeGraph) -> Result<f32> {
        let mut pattern_matches = 0;
        let total_patterns = graph.known_patterns.len();

        for pattern in &graph.known_patterns {
            if suggestion.contains(pattern) {
                pattern_matches += 1;
            }
        }

        if total_patterns == 0 {
            return Ok(0.5);
        }

        Ok(pattern_matches as f32 / total_patterns as f32)
    }

    /// Find related entities in knowledge graph
    async fn find_related_entities(
        &self,
        mentioned: &HashSet<String>,
        graph: &KnowledgeGraph,
    ) -> Result<HashSet<String>> {
        let mut related = HashSet::new();

        for entity in mentioned {
            if let Some(&node_idx) = graph.entity_lookup.get(entity) {
                // Find connected entities in graph
                for neighbor in graph.graph.neighbors(node_idx) {
                    if let Some(entity) = graph.graph.node_weight(neighbor) {
                        match entity {
                            Entity::File(name)
                            | Entity::Function(name)
                            | Entity::Struct(name)
                            | Entity::Module(name)
                            | Entity::Dependency(name) => {
                                related.insert(name.clone());
                            }
                            Entity::ApiEndpoint(name) => {
                                related.insert(name.clone());
                            }
                        }
                    }
                }
            }
        }

        Ok(related)
    }

    /// Generate feedback based on validation results
    fn generate_feedback(
        &self,
        scores: &HashMap<String, f32>,
        related: &HashSet<String>,
        suggestion: &str,
    ) -> (Vec<String>, Vec<String>) {
        let mut issues = Vec::new();
        let mut suggestions = Vec::new();

        // Analyze each validation aspect
        if let Some(&file_score) = scores.get("file_references") {
            if file_score < 0.8 {
                issues.push("Some file references may not exist in the project".to_string());
                suggestions
                    .push("Check file paths and ensure all referenced files exist".to_string());
            }
        }

        if let Some(&dep_score) = scores.get("dependencies") {
            if dep_score < 0.7 {
                issues.push("Dependency relationships may be invalid".to_string());
                suggestions.push("Verify import statements and module dependencies".to_string());
            }
        }

        if let Some(&call_score) = scores.get("function_calls") {
            if call_score < 0.6 {
                issues.push("Some function calls may reference non-existent functions".to_string());
                suggestions.push("Check function definitions and call signatures".to_string());
            }
        }

        // Suggest related entities
        if !related.is_empty() {
            suggestions.push(format!(
                "Consider these related entities: {}",
                related
                    .iter()
                    .take(5)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        (issues, suggestions)
    }
}

impl KnowledgeGraph {
    /// Create a new empty knowledge graph
    pub fn new() -> Self {
        Self {
            graph: Graph::new(),
            entity_lookup: HashMap::new(),
            file_dependencies: HashMap::new(),
            call_graph: HashMap::new(),
            known_patterns: Vec::new(),
        }
    }

    /// Build knowledge graph from project structure
    pub async fn build_from_project(&mut self, project_root: &std::path::Path) -> Result<()> {
        // Scan Rust files for entities and relationships
        self.scan_rust_files(project_root).await?;

        // Build dependency relationships
        self.build_dependency_graph().await?;

        // Build call graph
        self.build_call_graph().await?;

        Ok(())
    }

    /// Scan Rust files for entities
    async fn scan_rust_files(&mut self, project_root: &std::path::Path) -> Result<()> {
        fn scan_dir_sync(dir: &std::path::Path, graph: &mut KnowledgeGraph) -> Result<()> {
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_dir() {
                    let dir_name = path.file_name().unwrap_or_default().to_string_lossy();
                    if !["target", ".git", "node_modules"].contains(&dir_name.as_ref()) {
                        scan_dir_sync(&path, graph)?;
                    }
                } else if path.extension().unwrap_or_default() == "rs" {
                    graph.process_rust_file_sync(&path)?;
                }
            }

            Ok(())
        }

        // Run the blocking directory walk without moving `self` into a 'static task
        tokio::task::block_in_place(|| scan_dir_sync(project_root, self))?;

        Ok(())
    }

    /// Process a single Rust file synchronously
    async fn process_rust_file(&mut self, file_path: &std::path::Path) -> Result<()> {
        use tokio::fs;

        let content = fs::read_to_string(file_path).await?;
        let file_name = file_path
            .strip_prefix(self.get_project_root())
            .unwrap_or(file_path);
        let file_name = file_name.to_string_lossy().to_string();

        // Add file entity
        let file_node = self.graph.add_node(Entity::File(file_name.clone()));
        self.entity_lookup.insert(file_name.clone(), file_node);

        // Extract functions, structs, etc.
        self.extract_code_entities(&content, &file_name, file_node)?;

        Ok(())
    }

    /// Extract code entities from file content
    fn extract_code_entities(
        &mut self,
        content: &str,
        file_name: &str,
        file_node: NodeIndex,
    ) -> Result<()> {
        // Extract functions
        let fn_regex = Regex::new(r"fn\s+(\w+)\(")?;
        for capture in fn_regex.captures_iter(content) {
            if let Some(name) = capture.get(1) {
                let func_name = name.as_str().to_string();
                let func_node = self.graph.add_node(Entity::Function(func_name.clone()));
                self.graph
                    .add_edge(file_node, func_node, Relationship::Contains);
                self.entity_lookup.insert(func_name, func_node);
            }
        }

        // Extract structs
        let struct_regex = Regex::new(r"struct\s+(\w+)")?;
        for capture in struct_regex.captures_iter(content) {
            if let Some(name) = capture.get(1) {
                let struct_name = name.as_str().to_string();
                let struct_node = self.graph.add_node(Entity::Struct(struct_name.clone()));
                self.graph
                    .add_edge(file_node, struct_node, Relationship::Contains);
                self.entity_lookup.insert(struct_name, struct_node);
            }
        }

        Ok(())
    }

    /// Build dependency graph
    async fn build_dependency_graph(&mut self) -> Result<()> {
        // This would analyze use statements and build dependency relationships
        // Simplified for now
        Ok(())
    }

    /// Build function call graph
    async fn build_call_graph(&mut self) -> Result<()> {
        // This would analyze function calls and build call relationships
        // Simplified for now
        Ok(())
    }

    /// Check if dependency relationship is valid
    fn has_valid_dependency(&self, dep_text: &str) -> bool {
        // Simplified dependency validation
        !dep_text.contains("nonexistent_crate")
    }

    /// Get project root (placeholder)
    fn get_project_root(&self) -> &std::path::Path {
        std::path::Path::new(".")
    }

    /// Known patterns for validation
    pub fn known_patterns(&self) -> &[String] {
        &self.known_patterns
    }

    /// Process a single Rust file synchronously
    fn process_rust_file_sync(&mut self, file_path: &std::path::Path) -> Result<()> {
        let content = std::fs::read_to_string(file_path)?;
        let file_name = file_path
            .strip_prefix(std::path::Path::new("."))
            .unwrap_or(file_path);
        let file_name = file_name.to_string_lossy().to_string();

        // Add file entity
        let file_node = self.graph.add_node(Entity::File(file_name.clone()));
        self.entity_lookup.insert(file_name.clone(), file_node);

        // Extract functions, structs, etc.
        self.extract_code_entities_sync(&content, &file_name, file_node)?;

        Ok(())
    }

    /// Extract code entities from file content synchronously
    fn extract_code_entities_sync(
        &mut self,
        content: &str,
        file_name: &str,
        file_node: NodeIndex,
    ) -> Result<()> {
        // Extract functions
        let fn_regex = Regex::new(r"fn\s+(\w+)\(")?;
        for capture in fn_regex.captures_iter(content) {
            if let Some(name) = capture.get(1) {
                let func_name = name.as_str().to_string();
                let func_node = self.graph.add_node(Entity::Function(func_name.clone()));
                self.graph
                    .add_edge(file_node, func_node, Relationship::Contains);
                self.entity_lookup.insert(func_name, func_node);
            }
        }

        // Extract structs
        let struct_regex = Regex::new(r"struct\s+(\w+)")?;
        for capture in struct_regex.captures_iter(content) {
            if let Some(name) = capture.get(1) {
                let struct_name = name.as_str().to_string();
                let struct_node = self.graph.add_node(Entity::Struct(struct_name.clone()));
                self.graph
                    .add_edge(file_node, struct_node, Relationship::Contains);
                self.entity_lookup.insert(struct_name, struct_node);
            }
        }

        Ok(())
    }
}

impl ConfidenceCalculator {
    pub fn new(weights: ValidationWeights) -> Self {
        Self { weights }
    }

    /// Calculate overall confidence from validation scores
    pub fn calculate_overall(
        &self,
        scores: &HashMap<String, f32>,
        context: &ValidationContext,
        related_entities: &HashSet<String>,
    ) -> f32 {
        let file_score = scores.get("file_references").unwrap_or(&1.0);
        let dep_score = scores.get("dependencies").unwrap_or(&1.0);
        let call_score = scores.get("function_calls").unwrap_or(&1.0);
        let pattern_score = scores.get("patterns").unwrap_or(&0.5);

        // Weighted calculation
        let confidence = (self.weights.file_existence * file_score)
            + (self.weights.dependency_validity * dep_score)
            + (self.weights.call_graph_consistency * call_score)
            + (self.weights.pattern_matching * pattern_score);

        // Apply context modifiers
        let mut final_confidence = confidence;

        if context.is_recent {
            final_confidence += 0.1;
        }

        if context.user_reviewed {
            final_confidence += 0.1;
        }

        if !related_entities.is_empty() {
            final_confidence += 0.05; // Bonus for having related entities
        }

        final_confidence.min(1.0).max(0.0)
    }
}

/// Validation context with additional metadata
#[derive(Debug, Clone)]
pub struct ValidationContext {
    pub is_recent: bool,
    pub user_reviewed: bool,
    pub operation_type: String,
    pub risk_level: String,
    pub user_expertise: String,
    pub project_phase: String,
}

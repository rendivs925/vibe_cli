use regex::Regex;
use shared::types::Result;
use std::collections::HashMap;
use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator};

/// AST Parser for semantic code analysis
pub struct AstParser {
    parsers: HashMap<String, Parser>,
    language_queries: HashMap<String, HashMap<String, Query>>,
}

#[derive(Debug, Clone)]
pub struct AstNode {
    pub node_type: String,
    pub text: String,
    pub start_line: usize,
    pub end_line: usize,
    pub children: Vec<AstNode>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug)]
pub enum ParseError {
    UnsupportedLanguage(String),
    ParseFailed(String),
    QueryFailed(String),
}

impl AstParser {
    /// Create new AST parser with support for multiple languages
    pub fn new() -> Result<Self> {
        let mut parsers = HashMap::new();
        let mut language_queries = HashMap::new();

        // Initialize Rust parser
        let mut rust_parser = Parser::new();
        rust_parser.set_language(&tree_sitter_rust::LANGUAGE.into())?;
        parsers.insert("rs".to_string(), rust_parser);

        // Initialize Python parser
        let mut python_parser = Parser::new();
        python_parser.set_language(&tree_sitter_python::LANGUAGE.into())?;
        parsers.insert("py".to_string(), python_parser);

        // Initialize JavaScript parser
        let mut js_parser = Parser::new();
        js_parser.set_language(&tree_sitter_javascript::LANGUAGE.into())?;
        parsers.insert("js".to_string(), js_parser);

        // Initialize TypeScript parser
        let mut ts_parser = Parser::new();
        ts_parser.set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())?;
        parsers.insert("ts".to_string(), ts_parser);

        // Initialize language-specific queries
        Self::init_queries(&mut language_queries)?;

        Ok(Self {
            parsers,
            language_queries,
        })
    }

    /// Initialize language-specific queries for semantic extraction
    fn init_queries(queries: &mut HashMap<String, HashMap<String, Query>>) -> Result<()> {
        // Rust queries
        let mut rust_queries = HashMap::new();

        // Function definitions
        rust_queries.insert(
            "functions".to_string(),
            Query::new(
                &tree_sitter_rust::LANGUAGE.into(),
                r#"
            (function_item
                name: (identifier) @func_name
                parameters: (parameters) @func_params
                body: (block) @func_body) @function
            "#,
            )?,
        );

        // Struct definitions
        rust_queries.insert(
            "structs".to_string(),
            Query::new(
                &tree_sitter_rust::LANGUAGE.into(),
                r#"
            (struct_item
                name: (type_identifier) @struct_name
                body: (field_declaration_list) @struct_body) @struct
            "#,
            )?,
        );

        // Trait definitions
        rust_queries.insert(
            "traits".to_string(),
            Query::new(
                &tree_sitter_rust::LANGUAGE.into(),
                r#"
            (trait_item
                name: (type_identifier) @trait_name
                body: (declaration_list) @trait_body) @trait
            "#,
            )?,
        );

        queries.insert("rs".to_string(), rust_queries);

        // Python queries
        let mut python_queries = HashMap::new();

        // Function definitions
        python_queries.insert(
            "functions".to_string(),
            Query::new(
                &tree_sitter_python::LANGUAGE.into(),
                r#"
            (function_definition
                name: (identifier) @func_name
                parameters: (parameters) @func_params
                body: (block) @func_body) @function
            "#,
            )?,
        );

        // Class definitions
        python_queries.insert(
            "classes".to_string(),
            Query::new(
                &tree_sitter_python::LANGUAGE.into(),
                r#"
            (class_definition
                name: (identifier) @class_name
                body: (block) @class_body) @class
            "#,
            )?,
        );

        queries.insert("py".to_string(), python_queries);

        // JavaScript/TypeScript queries
        let mut js_queries = HashMap::new();

        // Function declarations
        js_queries.insert(
            "functions".to_string(),
            Query::new(
                &tree_sitter_javascript::LANGUAGE.into(),
                r#"
            [
                (function_declaration
                    name: (identifier) @func_name
                    parameters: (formal_parameters) @func_params
                    body: (statement_block) @func_body) @function
                (arrow_function
                    parameters: (formal_parameters) @func_params
                    body: (_) @func_body) @arrow_function
                (function_expression
                    name: (identifier)? @func_name
                    parameters: (formal_parameters) @func_params
                    body: (statement_block) @func_body) @function_expr
            ]
            "#,
            )?,
        );

        // Class declarations
        js_queries.insert(
            "classes".to_string(),
            Query::new(
                &tree_sitter_javascript::LANGUAGE.into(),
                r#"
            (class_declaration
                name: (identifier) @class_name
                body: (class_body) @class_body) @class
            "#,
            )?,
        );

        // Create TypeScript queries separately
        let mut ts_queries = HashMap::new();
        ts_queries.insert(
            "functions".to_string(),
            Query::new(
                &tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
                r#"
            [
                (function_declaration
                    name: (identifier) @func_name
                    parameters: (formal_parameters) @func_params
                    body: (statement_block) @func_body) @function
                (arrow_function
                    parameters: (formal_parameters) @func_params
                    body: (_) @func_body) @arrow_function
                (function_expression
                    name: (identifier)? @func_name
                    parameters: (formal_parameters) @func_params
                    body: (statement_block) @func_body) @function_expr
            ]
            "#,
            )?,
        );

        ts_queries.insert(
            "classes".to_string(),
            Query::new(
                &tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
                r#"
            (class_declaration
                name: (identifier) @class_name
                body: (class_body) @class_body) @class
            "#,
            )?,
        );

        queries.insert("js".to_string(), js_queries);
        queries.insert("ts".to_string(), ts_queries);

        Ok(())
    }

    /// Parse source code and extract semantic information
    pub fn parse_code(&mut self, code: &str, language: &str) -> Result<AstNode> {
        let parser = self
            .parsers
            .get_mut(language)
            .ok_or_else(|| anyhow::anyhow!("Unsupported language: {}", language))?;

        let tree = parser
            .parse(code, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse code"))?;

        let root_node = tree.root_node();
        let ast_node = self.extract_node_info(&root_node, code);

        Ok(ast_node)
    }

    /// Extract semantic chunks from parsed AST
    pub fn extract_semantic_chunks(&mut self, code: &str, language: &str) -> Result<Vec<String>> {
        let mut chunks = Vec::new();

        // Try AST-based extraction first
        match self.extract_chunks_ast(code, language) {
            Ok(ast_chunks) => {
                chunks.extend(ast_chunks);
            }
            Err(e) => {
                eprintln!("AST extraction failed, falling back to regex: {}", e);
                // Fallback to regex-based extraction
                chunks.extend(self.extract_chunks_regex(code, language));
            }
        }

        Ok(chunks)
    }

    /// Extract chunks using AST queries
    fn extract_chunks_ast(&mut self, code: &str, language: &str) -> Result<Vec<String>> {
        let parser = self
            .parsers
            .get_mut(language)
            .ok_or_else(|| anyhow::anyhow!("Unsupported language: {}", language))?;

        let tree = parser
            .parse(code, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse code"))?;

        let queries = self
            .language_queries
            .get(language)
            .ok_or_else(|| anyhow::anyhow!("No queries for language: {}", language))?;

        let mut chunks = Vec::new();

        for (query_type, query) in queries {
            let mut cursor = QueryCursor::new();
            let mut matches_iter = cursor.matches(query, tree.root_node(), code.as_bytes());

            while let Some(m) = matches_iter.next() {
                for capture in m.captures {
                    let node = capture.node;
                    let text = node.utf8_text(code.as_bytes())?;

                    // Extract meaningful chunks (functions, classes, etc.)
                    if Self::is_meaningful_chunk(query_type, text) {
                        chunks.push(text.to_string());
                    }
                }
            }
        }

        Ok(chunks)
    }

    /// Check if a chunk is meaningful for semantic analysis
    fn is_meaningful_chunk(query_type: &str, text: &str) -> bool {
        match query_type {
            "functions" => text.lines().count() >= 3, // At least function signature + body
            "classes" | "structs" | "traits" => text.lines().count() >= 2,
            _ => text.len() > 50, // General meaningful content threshold
        }
    }

    /// Fallback regex-based chunk extraction
    fn extract_chunks_regex(&self, code: &str, language: &str) -> Vec<String> {
        let patterns = self.get_regex_patterns(language);
        let mut chunks = Vec::new();

        for pattern in patterns {
            if let Ok(regex) = Regex::new(&pattern) {
                for capture in regex.captures_iter(code) {
                    if let Some(matched) = capture.get(0) {
                        let chunk = matched.as_str().trim();
                        if chunk.lines().count() >= 2 && chunk.len() > 30 {
                            chunks.push(chunk.to_string());
                        }
                    }
                }
            }
        }

        chunks
    }

    /// Get regex patterns for fallback parsing
    fn get_regex_patterns(&self, language: &str) -> Vec<String> {
        match language {
            "rs" => vec![
                r#"fn\s+\w+\([^}]*\)\s*\{[^}]*\}"#.to_string(), // Functions
                r#"struct\s+\w+[^}]*\}"#.to_string(),           // Structs
                r#"impl[^}]*\}"#.to_string(),                   // Implementations
                r#"trait\s+\w+[^}]*\}"#.to_string(),            // Traits
            ],
            "py" => vec![
                r#"def\s+\w+\([^)]*\):(?:\n\s+.*)*"#.to_string(), // Functions
                r#"class\s+\w+[^:]*:(?:\n\s+.*)*"#.to_string(),   // Classes
            ],
            "js" | "ts" => vec![
                r#"function\s+\w+\([^}]*\)\s*\{[^}]*\}"#.to_string(), // Functions
                r#"\w+\s*\([^}]*\)\s*=>\s*\{[^}]*\}"#.to_string(),    // Arrow functions
                r#"class\s+\w+[^}]*\}"#.to_string(),                  // Classes
            ],
            _ => vec![r#".*"#.to_string()], // Catch-all
        }
    }

    /// Extract node information recursively
    fn extract_node_info(&self, node: &tree_sitter::Node, source: &str) -> AstNode {
        let start = node.start_position();
        let end = node.end_position();

        let text = node.utf8_text(source.as_bytes()).unwrap_or("").to_string();

        let mut children = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            // Only include meaningful children (skip comments, whitespace)
            if !Self::is_noise_node(child.kind()) {
                children.push(self.extract_node_info(&child, source));
            }
        }

        AstNode {
            node_type: node.kind().to_string(),
            text,
            start_line: start.row,
            end_line: end.row,
            children,
            metadata: HashMap::new(),
        }
    }

    /// Check if a node type represents noise (comments, whitespace, etc.)
    fn is_noise_node(node_type: &str) -> bool {
        matches!(
            node_type,
            "comment"
                | "line_comment"
                | "block_comment"
                | "whitespace"
                | ";"
                | ","
                | "("
                | ")"
                | "{"
                | "}"
                | "["
                | "]"
                | ":"
                | "."
        )
    }

    /// Get supported languages
    pub fn supported_languages(&self) -> Vec<String> {
        self.parsers.keys().cloned().collect()
    }

    /// Get language statistics
    pub fn get_language_stats(
        &mut self,
        code: &str,
        language: &str,
    ) -> Result<HashMap<String, usize>> {
        let ast = self.parse_code(code, language)?;
        let mut stats = HashMap::new();

        Self::collect_stats(&ast, &mut stats);
        Ok(stats)
    }

    /// Recursively collect statistics from AST
    fn collect_stats(node: &AstNode, stats: &mut HashMap<String, usize>) {
        *stats.entry(node.node_type.clone()).or_insert(0) += 1;

        for child in &node.children {
            Self::collect_stats(child, stats);
        }
    }

    /// Extract code documentation and comments
    pub fn extract_documentation(&mut self, code: &str, language: &str) -> Result<Vec<String>> {
        let parser = self
            .parsers
            .get_mut(language)
            .ok_or_else(|| anyhow::anyhow!("Unsupported language: {}", language))?;

        let tree = parser
            .parse(code, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse code"))?;

        let query_str = match language {
            "rs" => r#"(line_comment) @comment"#,
            "py" => r#"(comment) @comment"#,
            "js" | "ts" => r#"(comment) @comment"#,
            _ => return Ok(vec![]),
        };

        let language_ref: &Language = match language {
            "rs" => &tree_sitter_rust::LANGUAGE.into(),
            "py" => &tree_sitter_python::LANGUAGE.into(),
            "js" => &tree_sitter_javascript::LANGUAGE.into(),
            "ts" => &tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            _ => return Ok(vec![]),
        };

        let query = Query::new(language_ref, query_str)?;
        let mut cursor = QueryCursor::new();
        let mut matches_iter = cursor.matches(&query, tree.root_node(), code.as_bytes());

        let mut docs = Vec::new();
        while let Some(m) = matches_iter.next() {
            for capture in m.captures {
                let node = capture.node;
                if let Ok(text) = node.utf8_text(code.as_bytes()) {
                    let clean_text = text.trim_start_matches("//").trim_start_matches("#").trim();
                    if !clean_text.is_empty() {
                        docs.push(clean_text.to_string());
                    }
                }
            }
        }

        Ok(docs)
    }
}

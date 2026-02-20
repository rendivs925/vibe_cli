use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct FunctionInfo {
    pub name: String,
    pub signature: String,
    pub line_start: usize,
    pub line_end: usize,
    pub is_public: bool,
    pub is_async: bool,
    pub params: Vec<String>,
    pub return_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StructInfo {
    pub name: String,
    pub fields: Vec<FieldInfo>,
    pub methods: Vec<String>,
    pub line_start: usize,
}

#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub name: String,
    pub field_type: String,
}

pub struct CodeAnalyzer {
    language: String,
}

impl CodeAnalyzer {
    pub fn new(language: String) -> Self {
        Self { language }
    }

    pub fn analyze_file(
        &self,
        file_path: &str,
    ) -> Result<HashMap<String, Vec<FunctionInfo>>, String> {
        let content = std::fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read file: {}", e))?;

        match self.language.as_str() {
            "rust" => self.analyze_rust(&content),
            "javascript" | "typescript" => self.analyze_js(&content),
            "python" => self.analyze_python(&content),
            _ => Ok(HashMap::new()),
        }
    }

    fn analyze_rust(&self, content: &str) -> Result<HashMap<String, Vec<FunctionInfo>>, String> {
        let mut functions: HashMap<String, Vec<FunctionInfo>> = HashMap::new();
        let mut current_mod = "root".to_string();

        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();

            if trimmed.starts_with("pub mod ") {
                let name = trimmed
                    .strip_prefix("pub mod ")
                    .and_then(|s| s.strip_suffix(';'))
                    .unwrap_or("")
                    .trim();
                current_mod = name.to_string();
                functions.entry(current_mod.clone()).or_default();
            }

            if trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ") {
                if let Some(func_info) = self.parse_rust_function(trimmed, i + 1) {
                    functions
                        .entry(current_mod.clone())
                        .or_default()
                        .push(func_info);
                }
            }
        }

        Ok(functions)
    }

    fn parse_rust_function(&self, line: &str, line_num: usize) -> Option<FunctionInfo> {
        let is_pub = line.starts_with("pub fn ");
        let fn_start = if is_pub { 7 } else { 3 };

        let rest = line[fn_start..].trim();
        if let Some(name_end) = rest.find('(') {
            let name = rest[..name_end].to_string();
            let params_raw = rest[name_end..]
                .strip_prefix('(')
                .and_then(|s| s.strip_suffix(')'))
                .unwrap_or("");

            let params: Vec<String> = params_raw
                .split(',')
                .filter_map(|p| {
                    let p = p.trim();
                    if p.is_empty() {
                        None
                    } else {
                        Some(p.to_string())
                    }
                })
                .collect();

            let return_type = if rest.contains("->") {
                rest.split("->")
                    .nth(1)
                    .map(|r| r.split('{').next().unwrap_or(r).trim().to_string())
            } else {
                None
            };

            return Some(FunctionInfo {
                name,
                signature: line.to_string(),
                line_start: line_num,
                line_end: line_num,
                is_public: is_pub,
                is_async: line.contains("async fn"),
                params,
                return_type,
            });
        }

        None
    }

    fn analyze_js(&self, content: &str) -> Result<HashMap<String, Vec<FunctionInfo>>, String> {
        let mut functions: HashMap<String, Vec<FunctionInfo>> = HashMap::new();
        let mut current_class = "global".to_string();
        let mut brace_depth = 0;

        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();

            if trimmed.starts_with("class ") && !trimmed.contains('{') {
                if let Some(name) = trimmed
                    .strip_prefix("class ")
                    .and_then(|s| s.split_whitespace().next())
                {
                    current_class = name.to_string();
                    functions.entry(current_class.clone()).or_default();
                }
            }

            if trimmed.starts_with("function ") || trimmed.starts_with("async function ") {
                if let Some(func_info) = self.parse_js_function(trimmed, i + 1) {
                    functions
                        .entry(current_class.clone())
                        .or_default()
                        .push(func_info);
                }
            }

            if trimmed.contains("=>")
                && (trimmed.starts_with("const ")
                    || trimmed.starts_with("let ")
                    || trimmed.starts_with("var "))
            {
                if let Some(name) = self.parse_arrow_function_name(trimmed) {
                    functions
                        .entry(current_class.clone())
                        .or_default()
                        .push(FunctionInfo {
                            name,
                            signature: trimmed.to_string(),
                            line_start: i + 1,
                            line_end: i + 1,
                            is_public: true,
                            is_async: trimmed.contains("async"),
                            params: vec![],
                            return_type: None,
                        });
                }
            }

            brace_depth += line.matches('{').count() as i32;
            brace_depth -= line.matches('}').count() as i32;

            if brace_depth == 0 && trimmed.is_empty() {
                current_class = "global".to_string();
            }
        }

        Ok(functions)
    }

    fn parse_js_function(&self, line: &str, line_num: usize) -> Option<FunctionInfo> {
        let is_async = line.starts_with("async function ");
        let fn_start = if is_async { 16 } else { 9 };

        let rest = line[fn_start..].trim();
        if let Some(name_end) = rest.find('(') {
            let name = rest[..name_end].to_string();
            let params_raw = rest[name_end..]
                .strip_prefix('(')
                .and_then(|s| s.split(')').next())
                .unwrap_or("");

            let params: Vec<String> = params_raw
                .split(',')
                .filter_map(|p| {
                    let p = p.trim();
                    if p.is_empty() {
                        None
                    } else {
                        Some(p.to_string())
                    }
                })
                .collect();

            return Some(FunctionInfo {
                name,
                signature: line.to_string(),
                line_start: line_num,
                line_end: line_num,
                is_public: true,
                is_async,
                params,
                return_type: None,
            });
        }

        None
    }

    fn parse_arrow_function_name(&self, line: &str) -> Option<String> {
        if let Some(eq_pos) = line.find("=>") {
            let before = line[..eq_pos].trim();
            if let Some(name_start) = before.rfind(|c: char| !c.is_alphanumeric() && c != '_') {
                Some(before[name_start + 1..].trim().to_string())
            } else {
                Some(before.to_string())
            }
        } else {
            None
        }
    }

    fn analyze_python(&self, content: &str) -> Result<HashMap<String, Vec<FunctionInfo>>, String> {
        let mut functions: HashMap<String, Vec<FunctionInfo>> = HashMap::new();
        let mut current_class = "global".to_string();

        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();

            if trimmed.starts_with("class ") && trimmed.contains(':') {
                if let Some(name) = trimmed
                    .strip_prefix("class ")
                    .and_then(|s| s.split(':').next())
                    .and_then(|s| s.split('(').next())
                {
                    current_class = name.trim().to_string();
                    functions.entry(current_class.clone()).or_default();
                }
            }

            if trimmed.starts_with("def ") {
                if let Some(func_info) = self.parse_python_function(trimmed, i + 1) {
                    functions
                        .entry(current_class.clone())
                        .or_default()
                        .push(func_info);
                }
            }
        }

        Ok(functions)
    }

    fn parse_python_function(&self, line: &str, line_num: usize) -> Option<FunctionInfo> {
        let rest = line.strip_prefix("def ").unwrap();
        if let Some(name_end) = rest.find('(') {
            let name = rest[..name_end].to_string();
            let params_raw = rest[name_end..]
                .strip_prefix('(')
                .and_then(|s| s.split(')').next())
                .unwrap_or("");

            let params: Vec<String> = params_raw
                .split(',')
                .filter_map(|p| {
                    let p = p.trim();
                    if p.is_empty() {
                        None
                    } else {
                        Some(p.to_string())
                    }
                })
                .collect();

            let return_type = if rest.contains("->") {
                rest.split("->")
                    .nth(1)
                    .and_then(|r| r.split(':').next().map(|t| t.trim().to_string()))
            } else {
                None
            };

            return Some(FunctionInfo {
                name,
                signature: line.to_string(),
                line_start: line_num,
                line_end: line_num,
                is_public: true,
                is_async: line.contains("async def"),
                params,
                return_type,
            });
        }

        None
    }

    pub fn find_related_files(&self, file_path: &str) -> Vec<String> {
        let path = Path::new(file_path);
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let parent = path.parent();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let mut related = Vec::new();

        if let Some(parent_dir) = parent {
            if let Ok(entries) = std::fs::read_dir(parent_dir) {
                for entry in entries.flatten() {
                    let entry_path = entry.path();
                    if let Some(name) = entry_path.file_name().and_then(|n| n.to_str()) {
                        let is_related = name.contains(stem)
                            || name == format!("{}.test.{}", stem, ext)
                            || name == format!("{}_test.{}", stem, ext)
                            || name == format!("{}.spec.{}", stem, ext)
                            || name.starts_with("test_") && name.ends_with(&format!(".{}", ext))
                            || name.ends_with(&format!(".test.{}", ext));

                        if is_related {
                            related.push(name.to_string());
                        }
                    }
                }
            }
        }

        related
    }
}

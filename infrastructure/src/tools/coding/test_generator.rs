use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct TestGenerator {
    project_info: super::project_scanner::ProjectInfo,
}

impl TestGenerator {
    pub fn new(project_info: super::project_scanner::ProjectInfo) -> Self {
        Self { project_info }
    }

    pub fn generate_test_for_function(
        &self,
        function_name: &str,
        file_path: &str,
        function_signature: &str,
    ) -> Result<String> {
        let test_content = match self.project_info.language.as_str() {
            "rust" => self.generate_rust_test(function_name, file_path, function_signature),
            "javascript" | "typescript" => {
                self.generate_js_test(function_name, file_path, function_signature)
            }
            "python" => self.generate_python_test(function_name, file_path, function_signature),
            "go" => self.generate_go_test(function_name, file_path, function_signature),
            _ => format!("// Language not supported: {}", self.project_info.language),
        };

        Ok(test_content)
    }

    fn generate_rust_test(&self, fn_name: &str, file_path: &str, _sig: &str) -> String {
        format!(
            r#"#[cfg(test)]
mod tests {{
    use super::*;

    #[test]
    fn test_{fn_name}_basic() {{
        // TODO: Add test setup and assertions
        // Testing: {fn_name} from {file_path}
        todo!("Implement test for {fn_name}")
    }}

    #[test]
    fn test_{fn_name}_error_cases() {{
        // TODO: Add error case tests
        todo!("Implement error case tests for {fn_name}")
    }}
}}
"#,
            fn_name = fn_name,
            file_path = file_path
        )
    }

    fn generate_js_test(&self, fn_name: &str, file_path: &str, _sig: &str) -> String {
        let test_framework = self
            .project_info
            .test_framework
            .as_deref()
            .unwrap_or("jest");

        match test_framework {
            "vitest" | "jest" => format!(
                r#"describe('{fn_name}', () => {{
  beforeEach(() => {{
    // TODO: Add test setup
  }});

  it('should handle basic case', () => {{
    // TODO: Add assertions for {fn_name} from {file_path}
    expect(true).toBe(true);
  }});

  it('should handle edge cases', () => {{
    // TODO: Add edge case tests
    expect(true).toBe(true);
  }});
}});
"#,
                fn_name = fn_name,
                file_path = file_path
            ),
            _ => format!(
                r#"// Testing {fn_name} from {file_path}
// TODO: Add test implementation
describe('{fn_name}', () => {{
  it('should work', () => {{
    expect(true).toBe(true);
  }});
}});
"#,
                fn_name = fn_name,
                file_path = file_path
            ),
        }
    }

    fn generate_python_test(&self, fn_name: &str, file_path: &str, _sig: &str) -> String {
        format!(
            r#"import pytest
from {} import {}  # TODO: Update import


class Test{}:
    """Tests for {} from {}"""
    
    def setup_method(self):
        """Setup for each test method"""
        pass

    def test_{}_basic(self):
        """Test basic functionality"""
        # TODO: Add test implementation
        assert True

    def test_{}_edge_cases(self):
        """Test edge cases"""
        # TODO: Add edge case tests
        assert True

    def test_{}_error_handling(self):
        """Test error handling"""
        # TODO: Add error handling tests
        assert True
"#,
            self.get_python_module_path(file_path),
            fn_name,
            Self::to_pascal_case(fn_name),
            fn_name,
            file_path,
            fn_name,
            fn_name,
            fn_name
        )
    }

    fn generate_go_test(&self, fn_name: &str, file_path: &str, _sig: &str) -> String {
        format!(
            r#"package main

import (
    "testing"
)

// Test{fn_name} tests the {fn_name} function from {file_path}
func Test{fn_name}(t *testing.T) {{
    // TODO: Add test implementation
    t.Skip("Implement test for {fn_name}")
}}

// Test{fn_name}EdgeCases tests edge cases for {fn_name}
func Test{fn_name}EdgeCases(t *testing.T) {{
    // TODO: Add edge case tests
    t.Skip("Implement edge case tests for {file_path}")
}}
"#,
            fn_name = Self::to_pascal_case(fn_name),
            file_path = file_path
        )
    }

    fn get_python_module_path(&self, file_path: &str) -> String {
        let path = Path::new(file_path);
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            stem.replace("-", "_")
        } else {
            "module".to_string()
        }
    }

    fn to_pascal_case(s: &str) -> String {
        let mut result = String::new();
        let mut capitalize_next = true;

        for c in s.chars() {
            if c == '_' || c == '-' {
                capitalize_next = true;
            } else if capitalize_next {
                result.push(c.to_ascii_uppercase());
                capitalize_next = false;
            } else {
                result.push(c);
            }
        }

        result
    }

    pub fn find_test_file(&self, source_file: &str) -> Option<PathBuf> {
        let source_path = Path::new(source_file);
        let stem = source_path.file_stem()?.to_str()?;

        for test_dir in &self.project_info.test_dirs {
            for ext in &["rs", "js", "ts", "py", "go"] {
                let test_name: String = match *ext {
                    "rs" => "lib.rs".to_string(),
                    _ => format!("{}.test.{}", stem, ext),
                };

                let test_path = source_path
                    .parent()
                    .map(|p| p.join(test_dir).join(&test_name))
                    .unwrap_or_else(|| PathBuf::from(test_dir).join(&test_name));

                if test_path.exists() {
                    return Some(test_path);
                }
            }
        }

        None
    }

    pub fn suggest_test_location(&self, source_file: &str) -> PathBuf {
        let source_path = Path::new(source_file);
        let test_dir = self
            .project_info
            .test_dirs
            .first()
            .cloned()
            .unwrap_or_else(|| "tests".to_string());

        source_path
            .parent()
            .map(|p| p.join(&test_dir))
            .unwrap_or_else(|| PathBuf::from(&test_dir))
    }
}

pub fn run_tests(project_info: &super::project_scanner::ProjectInfo) -> Result<(String, bool)> {
    let test_cmd = super::project_scanner::infer_test_command(project_info);

    let output = Command::new("sh").arg("-c").arg(&test_cmd).output()?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    let success = output.status.success();
    let output = if success {
        stdout
    } else {
        format!("{}\n{}", stdout, stderr)
    };

    Ok((output, success))
}

use crate::tools::common::ensure_args_at_least;
use domain::tools::{Tool, ToolError, ToolOutput};
use quote::ToTokens;
use std::fs;

pub struct AstTool;

impl Tool for AstTool {
    fn name(&self) -> &str {
        "ast"
    }

    fn description(&self) -> &str {
        "Parse source file and summarize top-level AST nodes"
    }

    fn usage(&self) -> &str {
        "ast <path>"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["ast src/main.rs", "ast presentation/src/cli/handlers/react.rs"]
    }

    fn requires_confirmation(&self) -> bool {
        false
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 1, self.usage())?;
        let path = args[0];
        let content = fs::read_to_string(path)
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

        if path.ends_with(".rs") {
            return summarize_rust_ast(path, &content);
        }

        let mut out = ToolOutput::success(format!(
            "AST parsing currently supports Rust files only. Path: {path}"
        ));
        out.metadata
            .insert("language".to_string(), "unknown".to_string());
        Ok(out)
    }
}

fn summarize_rust_ast(path: &str, content: &str) -> Result<ToolOutput, ToolError> {
    let parsed =
        syn::parse_file(content).map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

    let mut lines = Vec::new();
    lines.push(format!("File: {path}"));

    for item in parsed.items {
        match item {
            syn::Item::Fn(item) => lines.push(format!("fn {}", item.sig.ident)),
            syn::Item::Struct(item) => lines.push(format!("struct {}", item.ident)),
            syn::Item::Enum(item) => lines.push(format!("enum {}", item.ident)),
            syn::Item::Trait(item) => lines.push(format!("trait {}", item.ident)),
            syn::Item::Impl(_) => lines.push("impl ...".to_string()),
            syn::Item::Mod(item) => lines.push(format!("mod {}", item.ident)),
            syn::Item::Use(item) => lines.push(format!("use {}", item.to_token_stream())),
            syn::Item::Const(item) => lines.push(format!("const {}", item.ident)),
            syn::Item::Type(item) => lines.push(format!("type {}", item.ident)),
            _ => {}
        }
    }

    if lines.len() == 1 {
        lines.push("(no top-level declarations found)".to_string());
    }

    let mut out = ToolOutput::success(lines.join("\n"));
    out.metadata
        .insert("language".to_string(), "rust".to_string());
    Ok(out)
}

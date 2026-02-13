use regex::Regex;
use std::fs;

pub struct AstSummary {
    pub language: String,
    pub summary: String,
}

pub fn summarize_file(path: &str) -> Option<AstSummary> {
    let content = fs::read_to_string(path).ok()?;
    summarize_source(path, &content)
}

pub fn summarize_source(path: &str, content: &str) -> Option<AstSummary> {
    let ext = path.rsplit('.').next()?.to_ascii_lowercase();
    match ext.as_str() {
        "rs" => summarize_rust(content),
        "py" => summarize_python(content),
        "js" | "jsx" | "mjs" | "cjs" => summarize_js_like(content, "javascript"),
        "ts" | "tsx" => summarize_js_like(content, "typescript"),
        "go" => summarize_go(content),
        "java" => summarize_java(content),
        "c" | "h" | "hpp" | "cpp" | "cc" => summarize_c_family(content, &ext),
        _ => None,
    }
}

fn summarize_rust(content: &str) -> Option<AstSummary> {
    let parsed = syn::parse_file(content).ok()?;
    let mut fns = Vec::new();
    let mut structs = Vec::new();
    let mut enums = Vec::new();
    let mut traits = Vec::new();
    let mut mods = Vec::new();

    for item in parsed.items {
        match item {
            syn::Item::Fn(item) => fns.push(item.sig.ident.to_string()),
            syn::Item::Struct(item) => structs.push(item.ident.to_string()),
            syn::Item::Enum(item) => enums.push(item.ident.to_string()),
            syn::Item::Trait(item) => traits.push(item.ident.to_string()),
            syn::Item::Mod(item) => mods.push(item.ident.to_string()),
            _ => {}
        }
    }

    let mut lines = Vec::new();
    lines.push("RUST AST SUMMARY".to_string());
    push_names(&mut lines, "modules", &mods);
    push_names(&mut lines, "structs", &structs);
    push_names(&mut lines, "enums", &enums);
    push_names(&mut lines, "traits", &traits);
    push_names(&mut lines, "functions", &fns);

    Some(AstSummary {
        language: "rust".to_string(),
        summary: lines.join("\n"),
    })
}

fn summarize_python(content: &str) -> Option<AstSummary> {
    let class_re = Regex::new(r"(?m)^\s*class\s+([A-Za-z_][A-Za-z0-9_]*)").ok()?;
    let fn_re = Regex::new(r"(?m)^\s*def\s+([A-Za-z_][A-Za-z0-9_]*)").ok()?;
    let import_re = Regex::new(r"(?m)^\s*(?:from\s+\S+\s+import|import\s+\S+)").ok()?;

    let classes = captures(&class_re, content);
    let functions = captures(&fn_re, content);
    let imports = import_re.find_iter(content).count();

    Some(AstSummary {
        language: "python".to_string(),
        summary: format_summary("PYTHON STRUCTURE SUMMARY", &classes, &functions, imports),
    })
}

fn summarize_js_like(content: &str, lang: &str) -> Option<AstSummary> {
    let class_re = Regex::new(r"(?m)^\s*class\s+([A-Za-z_][A-Za-z0-9_]*)").ok()?;
    let fn_re = Regex::new(
        r"(?m)^\s*(?:async\s+)?function\s+([A-Za-z_][A-Za-z0-9_]*)|^\s*const\s+([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(?:async\s*)?\(",
    )
    .ok()?;
    let import_re = Regex::new(r"(?m)^\s*(?:import\s+.+\s+from\s+|const\s+.+\s*=\s*require\()")
        .ok()?;

    let classes = captures(&class_re, content);
    let mut functions = Vec::new();
    for caps in fn_re.captures_iter(content) {
        if let Some(name) = caps.get(1).or_else(|| caps.get(2)) {
            functions.push(name.as_str().to_string());
        }
    }
    let imports = import_re.find_iter(content).count();

    Some(AstSummary {
        language: lang.to_string(),
        summary: format_summary(
            &format!("{} STRUCTURE SUMMARY", lang.to_uppercase()),
            &classes,
            &functions,
            imports,
        ),
    })
}

fn summarize_go(content: &str) -> Option<AstSummary> {
    let type_re = Regex::new(r"(?m)^\s*type\s+([A-Za-z_][A-Za-z0-9_]*)\s+").ok()?;
    let fn_re = Regex::new(r"(?m)^\s*func\s+(?:\([^)]*\)\s*)?([A-Za-z_][A-Za-z0-9_]*)").ok()?;
    let import_re = Regex::new(r#"(?m)^\s*import\s+(?:\(|")"#).ok()?;

    let types = captures(&type_re, content);
    let functions = captures(&fn_re, content);
    let imports = import_re.find_iter(content).count();

    Some(AstSummary {
        language: "go".to_string(),
        summary: format_summary("GO STRUCTURE SUMMARY", &types, &functions, imports),
    })
}

fn summarize_java(content: &str) -> Option<AstSummary> {
    let class_re = Regex::new(
        r"(?m)^\s*(?:public\s+)?(?:abstract\s+)?(?:class|interface|enum)\s+([A-Za-z_][A-Za-z0-9_]*)",
    )
    .ok()?;
    let method_re = Regex::new(
        r"(?m)^\s*(?:public|private|protected)?\s*(?:static\s+)?[A-Za-z_<>,\[\]]+\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(",
    )
    .ok()?;
    let import_re = Regex::new(r"(?m)^\s*import\s+").ok()?;

    let classes = captures(&class_re, content);
    let methods = captures(&method_re, content);
    let imports = import_re.find_iter(content).count();

    Some(AstSummary {
        language: "java".to_string(),
        summary: format_summary("JAVA STRUCTURE SUMMARY", &classes, &methods, imports),
    })
}

fn summarize_c_family(content: &str, ext: &str) -> Option<AstSummary> {
    let struct_re = Regex::new(r"(?m)^\s*(?:typedef\s+)?struct\s+([A-Za-z_][A-Za-z0-9_]*)").ok()?;
    let fn_re = Regex::new(r"(?m)^\s*[A-Za-z_][A-Za-z0-9_\s\*]+\s+([A-Za-z_][A-Za-z0-9_]*)\s*\([^;]*\)\s*\{").ok()?;
    let include_re = Regex::new(r"(?m)^\s*#include\s+").ok()?;

    let structs = captures(&struct_re, content);
    let functions = captures(&fn_re, content);
    let imports = include_re.find_iter(content).count();

    Some(AstSummary {
        language: ext.to_string(),
        summary: format_summary("C/C++ STRUCTURE SUMMARY", &structs, &functions, imports),
    })
}

fn captures(re: &Regex, content: &str) -> Vec<String> {
    re.captures_iter(content)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

fn format_summary(title: &str, types: &[String], funcs: &[String], imports: usize) -> String {
    let mut lines = Vec::new();
    lines.push(title.to_string());
    lines.push(format!("imports: {imports}"));
    push_names(&mut lines, "types", types);
    push_names(&mut lines, "functions", funcs);
    lines.join("\n")
}

fn push_names(lines: &mut Vec<String>, label: &str, names: &[String]) {
    if names.is_empty() {
        lines.push(format!("{label}: 0"));
        return;
    }

    let preview = names.iter().take(24).cloned().collect::<Vec<_>>().join(", ");
    lines.push(format!("{label}: {}", names.len()));
    lines.push(format!("{label}_names: {preview}"));
}

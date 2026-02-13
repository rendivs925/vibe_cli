use std::fs;
use tree_sitter::{Language, Node, Parser, Tree};

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
        "py" => summarize_with_tree_sitter(content, LanguageProfile::python()),
        "js" | "jsx" | "mjs" | "cjs" => {
            summarize_with_tree_sitter(content, LanguageProfile::javascript())
        }
        "ts" => summarize_with_tree_sitter(content, LanguageProfile::typescript()),
        "tsx" => summarize_with_tree_sitter(content, LanguageProfile::tsx()),
        "go" => summarize_with_tree_sitter(content, LanguageProfile::go()),
        "java" => summarize_with_tree_sitter(content, LanguageProfile::java()),
        "c" | "h" => summarize_with_tree_sitter(content, LanguageProfile::c()),
        "cpp" | "cc" | "cxx" | "hpp" => {
            summarize_with_tree_sitter(content, LanguageProfile::cpp())
        }
        _ => None,
    }
}

fn summarize_rust(content: &str) -> Option<AstSummary> {
    let parsed = syn::parse_file(content).ok()?;
    let mut modules = Vec::new();
    let mut structs = Vec::new();
    let mut enums = Vec::new();
    let mut traits = Vec::new();
    let mut functions = Vec::new();

    for item in parsed.items {
        match item {
            syn::Item::Mod(item) => modules.push(item.ident.to_string()),
            syn::Item::Struct(item) => structs.push(item.ident.to_string()),
            syn::Item::Enum(item) => enums.push(item.ident.to_string()),
            syn::Item::Trait(item) => traits.push(item.ident.to_string()),
            syn::Item::Fn(item) => functions.push(item.sig.ident.to_string()),
            _ => {}
        }
    }

    let mut lines = Vec::new();
    lines.push("RUST AST SUMMARY".to_string());
    push_names(&mut lines, "modules", &modules);
    push_names(&mut lines, "structs", &structs);
    push_names(&mut lines, "enums", &enums);
    push_names(&mut lines, "traits", &traits);
    push_names(&mut lines, "functions", &functions);

    Some(AstSummary {
        language: "rust".to_string(),
        summary: lines.join("\n"),
    })
}

struct LanguageProfile {
    name: &'static str,
    language: Language,
    function_kinds: &'static [&'static str],
    type_kinds: &'static [&'static str],
    name_kinds: &'static [&'static str],
}

impl LanguageProfile {
    fn python() -> Self {
        Self {
            name: "python",
            language: tree_sitter_python::LANGUAGE.into(),
            function_kinds: &["function_definition"],
            type_kinds: &["class_definition"],
            name_kinds: &["identifier"],
        }
    }

    fn javascript() -> Self {
        Self {
            name: "javascript",
            language: tree_sitter_javascript::LANGUAGE.into(),
            function_kinds: &[
                "function_declaration",
                "method_definition",
                "generator_function_declaration",
                "arrow_function",
            ],
            type_kinds: &["class_declaration"],
            name_kinds: &["identifier", "property_identifier"],
        }
    }

    fn typescript() -> Self {
        Self {
            name: "typescript",
            language: tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            function_kinds: &[
                "function_declaration",
                "method_definition",
                "abstract_method_signature",
                "arrow_function",
            ],
            type_kinds: &[
                "class_declaration",
                "interface_declaration",
                "type_alias_declaration",
                "enum_declaration",
            ],
            name_kinds: &["identifier", "type_identifier", "property_identifier"],
        }
    }

    fn tsx() -> Self {
        Self {
            name: "tsx",
            language: tree_sitter_typescript::LANGUAGE_TSX.into(),
            function_kinds: &[
                "function_declaration",
                "method_definition",
                "abstract_method_signature",
                "arrow_function",
            ],
            type_kinds: &[
                "class_declaration",
                "interface_declaration",
                "type_alias_declaration",
                "enum_declaration",
            ],
            name_kinds: &["identifier", "type_identifier", "property_identifier"],
        }
    }

    fn go() -> Self {
        Self {
            name: "go",
            language: tree_sitter_go::LANGUAGE.into(),
            function_kinds: &["function_declaration", "method_declaration"],
            type_kinds: &["type_declaration", "type_spec"],
            name_kinds: &["identifier", "type_identifier", "field_identifier"],
        }
    }

    fn java() -> Self {
        Self {
            name: "java",
            language: tree_sitter_java::LANGUAGE.into(),
            function_kinds: &["method_declaration", "constructor_declaration"],
            type_kinds: &[
                "class_declaration",
                "interface_declaration",
                "enum_declaration",
                "annotation_type_declaration",
            ],
            name_kinds: &["identifier"],
        }
    }

    fn c() -> Self {
        Self {
            name: "c",
            language: tree_sitter_c::LANGUAGE.into(),
            function_kinds: &["function_definition"],
            type_kinds: &["struct_specifier", "enum_specifier", "union_specifier"],
            name_kinds: &["identifier", "type_identifier", "field_identifier"],
        }
    }

    fn cpp() -> Self {
        Self {
            name: "cpp",
            language: tree_sitter_cpp::LANGUAGE.into(),
            function_kinds: &["function_definition", "function_declarator"],
            type_kinds: &[
                "class_specifier",
                "struct_specifier",
                "enum_specifier",
                "union_specifier",
            ],
            name_kinds: &["identifier", "type_identifier", "field_identifier"],
        }
    }
}

fn summarize_with_tree_sitter(content: &str, profile: LanguageProfile) -> Option<AstSummary> {
    let mut parser = Parser::new();
    parser.set_language(&profile.language).ok()?;
    let tree = parser.parse(content, None)?;

    let mut functions = Vec::new();
    let mut types = Vec::new();
    walk_tree(
        &tree,
        content,
        &profile,
        &mut functions,
        &mut types,
    );

    let mut lines = Vec::new();
    lines.push(format!("{} AST SUMMARY", profile.name.to_uppercase()));
    push_names(&mut lines, "types", &types);
    push_names(&mut lines, "functions", &functions);

    Some(AstSummary {
        language: profile.name.to_string(),
        summary: lines.join("\n"),
    })
}

fn walk_tree(
    tree: &Tree,
    source: &str,
    profile: &LanguageProfile,
    functions: &mut Vec<String>,
    types: &mut Vec<String>,
) {
    let mut cursor = tree.walk();
    let mut stack = vec![tree.root_node()];

    while let Some(node) = stack.pop() {
        let kind = node.kind();

        if profile.function_kinds.contains(&kind) {
            if let Some(name) = extract_name(node, source, profile.name_kinds) {
                functions.push(name);
            }
        }

        if profile.type_kinds.contains(&kind) {
            if let Some(name) = extract_name(node, source, profile.name_kinds) {
                types.push(name);
            }
        }

        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }
}

fn extract_name(node: Node<'_>, source: &str, name_kinds: &[&str]) -> Option<String> {
    if let Some(name_node) = node.child_by_field_name("name") {
        if let Ok(text) = name_node.utf8_text(source.as_bytes()) {
            let cleaned = text.trim();
            if !cleaned.is_empty() {
                return Some(cleaned.to_string());
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if name_kinds.contains(&child.kind()) {
            if let Ok(text) = child.utf8_text(source.as_bytes()) {
                let cleaned = text.trim();
                if !cleaned.is_empty() {
                    return Some(cleaned.to_string());
                }
            }
        }
    }

    None
}

fn push_names(lines: &mut Vec<String>, label: &str, names: &[String]) {
    if names.is_empty() {
        lines.push(format!("{label}: 0"));
        return;
    }

    let mut unique = names.to_vec();
    unique.sort();
    unique.dedup();
    let preview = unique.iter().take(40).cloned().collect::<Vec<_>>().join(", ");

    lines.push(format!("{label}: {}", unique.len()));
    lines.push(format!("{label}_names: {preview}"));
}

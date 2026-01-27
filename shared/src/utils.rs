use std::path::Path;

pub fn is_supported_file(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    matches!(
        ext,
        "rs" | "md"
            | "toml"
            | "json"
            | "graphql"
            | "c"
            | "h"
            | "cpp"
            | "hpp"
            | "cc"
            | "cxx"
            | "py"
            | "js"
            | "ts"
            | "java"
            | "go"
            | "rb"
            | "php"
            | "sh"
            | "bash"
            | "zsh"
            | "fish"
            | "html"
            | "css"
            | "scss"
            | "sass"
            | "xml"
            | "yaml"
            | "yml"
            | "ini"
            | "cfg"
            | "conf"
    )
}

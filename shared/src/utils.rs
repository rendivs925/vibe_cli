use std::path::Path;

pub fn is_supported_file(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    matches!(ext, "rs" | "md" | "toml" | "json" | "graphql")
}

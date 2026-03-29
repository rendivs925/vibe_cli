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

pub trait StringExt {
    fn trimmed(&self) -> String;
    fn trimmed_lower(&self) -> String;
    fn trimmed_upper(&self) -> String;
    fn is_blank(&self) -> bool;
    fn if_empty(&self, default: &str) -> String;
}

impl StringExt for str {
    fn trimmed(&self) -> String {
        self.trim().to_string()
    }

    fn trimmed_lower(&self) -> String {
        self.trim().to_lowercase()
    }

    fn trimmed_upper(&self) -> String {
        self.trim().to_uppercase()
    }

    fn is_blank(&self) -> bool {
        self.trim().is_empty()
    }

    fn if_empty(&self, default: &str) -> String {
        let s = self.trim();
        if s.is_empty() {
            default.to_string()
        } else {
            s.to_string()
        }
    }
}

pub fn parse_bool(value: &str) -> Option<bool> {
    let v = value.trim().to_lowercase();
    if v == "true" || v == "1" || v == "yes" || v == "on" {
        Some(true)
    } else if v == "false" || v == "0" || v == "no" || v == "off" {
        Some(false)
    } else {
        None
    }
}

pub fn parse_number(value: &str) -> Option<f64> {
    value.trim().parse::<f64>().ok()
}

pub fn parse_int(value: &str) -> Option<i64> {
    value.trim().parse::<i64>().ok()
}

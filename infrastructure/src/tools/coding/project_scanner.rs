use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ProjectInfo {
    pub language: String,
    pub framework: Option<String>,
    pub test_framework: Option<String>,
    pub build_system: Option<String>,
    pub package_manager: Option<String>,
    pub has_docker: bool,
    pub has_git: bool,
    pub root_files: Vec<String>,
    pub src_dirs: Vec<String>,
    pub test_dirs: Vec<String>,
    pub config_files: HashMap<String, String>,
}

impl ProjectInfo {
    pub fn new() -> Self {
        Self {
            language: "unknown".to_string(),
            framework: None,
            test_framework: None,
            build_system: None,
            package_manager: None,
            has_docker: false,
            has_git: false,
            root_files: Vec::new(),
            src_dirs: Vec::new(),
            test_dirs: Vec::new(),
            config_files: HashMap::new(),
        }
    }

    pub fn to_json(&self) -> String {
        let mut json = String::from("{");
        json.push_str(&format!("\"language\": \"{}\",", self.language));

        if let Some(ref f) = self.framework {
            json.push_str(&format!("\"framework\": \"{}\",", f));
        } else {
            json.push_str("\"framework\": null,");
        }

        if let Some(ref t) = self.test_framework {
            json.push_str(&format!("\"test_framework\": \"{}\",", t));
        } else {
            json.push_str("\"test_framework\": null,");
        }

        json.push_str(&format!("\"has_docker\": {},", self.has_docker));
        json.push_str(&format!("\"has_git\": {},", self.has_git));
        json.push_str(&format!("\"root_files\": {:?},", self.root_files));
        json.push_str(&format!("\"src_dirs\": {:?},", self.src_dirs));
        json.push_str(&format!("\"test_dirs\": {:?},", self.test_dirs));

        json.push_str("\"config_files\": {");
        let mut first = true;
        for (k, v) in &self.config_files {
            if !first {
                json.push_str(",");
            }
            json.push_str(&format!("\"{}\": \"{}\"", k, v));
            first = false;
        }
        json.push_str("}}");

        json
    }
}

pub fn scan_project(root: &Path) -> ProjectInfo {
    let mut info = ProjectInfo::new();

    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                info.root_files.push(name.to_string());

                match name {
                    "Cargo.toml" => {
                        info.language = "rust".to_string();
                        info.build_system = Some("cargo".to_string());
                        info.test_framework = Some("cargo test".to_string());
                        info.config_files
                            .insert("cargo".to_string(), read_file_content(&path));
                    }
                    "package.json" => {
                        info.language = "javascript".to_string();
                        info.package_manager = Some("npm".to_string());
                        info.test_framework = Some("jest".to_string());
                        read_package_json(&path, &mut info);
                    }
                    "pyproject.toml" | "setup.py" | "requirements.txt" => {
                        info.language = "python".to_string();
                        info.package_manager = Some("pip".to_string());
                        info.test_framework = Some("pytest".to_string());
                    }
                    "go.mod" => {
                        info.language = "go".to_string();
                        info.build_system = Some("go".to_string());
                        info.test_framework = Some("go test".to_string());
                    }
                    "pom.xml" => {
                        info.language = "java".to_string();
                        info.build_system = Some("maven".to_string());
                        info.test_framework = Some("junit".to_string());
                    }
                    "build.gradle" => {
                        info.language = "java".to_string();
                        info.build_system = Some("gradle".to_string());
                        info.test_framework = Some("junit".to_string());
                    }
                    "Dockerfile" => {
                        info.has_docker = true;
                    }
                    ".git" => {
                        info.has_git = true;
                    }
                    _ => {}
                }
            }
        }
    }

    detect_framework(&mut info);
    find_src_dirs(root, &mut info);
    find_test_dirs(root, &mut info);

    info
}

fn read_file_content(path: &Path) -> String {
    std::fs::read_to_string(path)
        .map(|s| s.chars().take(500).collect())
        .unwrap_or_default()
}

fn read_package_json(path: &Path, info: &mut ProjectInfo) {
    if let Ok(content) = std::fs::read_to_string(path) {
        info.config_files
            .insert("package.json".to_string(), content.clone());

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(deps) = json.get("dependencies").and_then(|d| d.as_object()) {
                for (key, _) in deps {
                    match key.as_str() {
                        "react" | "vue" | "angular" => {
                            info.framework = Some(key.clone());
                        }
                        "express" | "fastify" | "koa" => {
                            info.framework = Some(format!("express-like"));
                        }
                        "next" | "nuxt" | "sveltekit" => {
                            info.framework = Some(key.clone());
                        }
                        _ => {}
                    }
                }
            }

            if let Some(dev_deps) = json.get("devDependencies").and_then(|d| d.as_object()) {
                for (key, _) in dev_deps {
                    match key.as_str() {
                        "jest" => info.test_framework = Some("jest".to_string()),
                        "vitest" => info.test_framework = Some("vitest".to_string()),
                        "mocha" => info.test_framework = Some("mocha".to_string()),
                        "pytest" => info.test_framework = Some("pytest".to_string()),
                        "playwright" | "cypress" => {
                            info.test_framework = Some(key.clone());
                        }
                        "typescript" => {}
                        _ => {}
                    }
                }
            }
        }
    }
}

fn detect_framework(info: &mut ProjectInfo) {
    for file in &info.root_files {
        match file.as_str() {
            "Cargo.toml" => {
                if let Some(toml) = info.config_files.get("cargo") {
                    if toml.contains("actix-web") {
                        info.framework = Some("actix-web".to_string());
                    } else if toml.contains("tokio") {
                        info.framework = Some("tokio".to_string());
                    } else if toml.contains("warp") {
                        info.framework = Some("warp".to_string());
                    } else if toml.contains("axum") {
                        info.framework = Some("axum".to_string());
                    }
                }
            }
            "Gemfile" => {
                info.language = "ruby".to_string();
                info.framework = Some("rails".to_string());
            }
            "composer.json" => {
                info.language = "php".to_string();
            }
            _ => {}
        }
    }
}

fn find_src_dirs(root: &Path, info: &mut ProjectInfo) {
    let src_patterns = ["src", "lib", "app", "source", "sources"];

    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if src_patterns.iter().any(|p| name.eq_ignore_ascii_case(p)) {
                        info.src_dirs.push(name.to_string());
                    }
                }
            }
        }
    }

    if info.src_dirs.is_empty() {
        info.src_dirs.push(".".to_string());
    }
}

fn find_test_dirs(root: &Path, info: &mut ProjectInfo) {
    let test_patterns = ["tests", "test", "__tests__", "specs", "spec", "Tests"];

    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if test_patterns.iter().any(|p| name.eq_ignore_ascii_case(p)) {
                        info.test_dirs.push(name.to_string());
                    }
                }
            }
        }
    }
}

pub fn infer_run_command(info: &ProjectInfo) -> String {
    match info.language.as_str() {
        "rust" => "cargo run".to_string(),
        "javascript" | "typescript" => {
            if info
                .framework
                .as_ref()
                .map(|f| f.contains("next"))
                .unwrap_or(false)
            {
                "npm run dev".to_string()
            } else {
                "node index.js".to_string()
            }
        }
        "python" => "python main.py".to_string(),
        "go" => "go run .".to_string(),
        "java" => {
            if info
                .build_system
                .as_ref()
                .map(|b| b == "gradle")
                .unwrap_or(false)
            {
                "./gradlew run".to_string()
            } else {
                "mvn exec:java".to_string()
            }
        }
        _ => "echo 'No run command detected'".to_string(),
    }
}

pub fn infer_test_command(info: &ProjectInfo) -> String {
    match info.language.as_str() {
        "rust" => "cargo test".to_string(),
        "javascript" | "typescript" => {
            if let Some(ref tf) = info.test_framework {
                match tf.as_str() {
                    "jest" => "npm test".to_string(),
                    "vitest" => "npx vitest".to_string(),
                    "playwright" => "npx playwright test".to_string(),
                    "cypress" => "npx cypress run".to_string(),
                    _ => "npm test".to_string(),
                }
            } else {
                "npm test".to_string()
            }
        }
        "python" => "pytest".to_string(),
        "go" => "go test ./...".to_string(),
        "java" => {
            if info
                .build_system
                .as_ref()
                .map(|b| b == "gradle")
                .unwrap_or(false)
            {
                "./gradlew test".to_string()
            } else {
                "mvn test".to_string()
            }
        }
        _ => "echo 'No test command detected'".to_string(),
    }
}

pub fn infer_lint_command(info: &ProjectInfo) -> String {
    match info.language.as_str() {
        "rust" => "cargo clippy".to_string(),
        "javascript" | "typescript" => "npm run lint".to_string(),
        "python" => "ruff check .".to_string(),
        "go" => "golangci-lint run".to_string(),
        _ => "echo 'No lint command detected'".to_string(),
    }
}

pub fn infer_build_command(info: &ProjectInfo) -> String {
    match info.language.as_str() {
        "rust" => "cargo build".to_string(),
        "javascript" | "typescript" => "npm run build".to_string(),
        "python" => "python -m build".to_string(),
        "go" => "go build".to_string(),
        "java" => {
            if info
                .build_system
                .as_ref()
                .map(|b| b == "gradle")
                .unwrap_or(false)
            {
                "./gradlew build".to_string()
            } else {
                "mvn build".to_string()
            }
        }
        _ => "echo 'No build command detected'".to_string(),
    }
}

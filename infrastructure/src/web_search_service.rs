use serde::{Deserialize, Serialize};
use std::process::{Command, Output};
use std::thread::sleep as std_sleep;
use tokio::time::{sleep, Duration};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

pub struct WebSearchService {
    searxng_url: String,
}

const SEARXNG_CONTAINER_NAME: &str = "searxng";
const SEARXNG_HOST_PORT: u16 = 8888;
const SEARXNG_CONTAINER_PORT: u16 = 8080;
const SEARXNG_VOLUME_NAME: &str = "searxng-data-v2";

impl WebSearchService {
    pub fn new(searxng_url: String) -> Self {
        Self { searxng_url }
    }

    pub async fn search(
        &self,
        query: &str,
        num_results: usize,
    ) -> Result<Vec<SearchResult>, String> {
        let mut attempt = 0;
        let max_attempts = 5;
        let mut started = false;

        loop {
            attempt += 1;
            let search_url = self
                .get_active_searxng_url()
                .unwrap_or_else(|| self.searxng_url.clone());

            match self
                .searxng_search_with_url(&search_url, query, num_results)
                .await
            {
                Ok(results) => return Ok(results),
                Err(e) if Self::is_connection_error(&e) => {
                    if !started {
                        println!("SearXNG not running. Starting container...");
                        self.start_searxng()?;
                        started = true;
                    }

                    if attempt >= max_attempts {
                        return Err(e);
                    }

                    let backoff = 2_u64.saturating_pow(attempt.min(4) as u32);
                    sleep(Duration::from_secs(backoff)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    fn get_active_searxng_url(&self) -> Option<String> {
        let output = Command::new("docker")
            .args([
                "ps",
                "--filter",
                &format!("name={}", SEARXNG_CONTAINER_NAME),
                "--format",
                "{{.Ports}}",
            ])
            .output()
            .ok()?;

        let ports = String::from_utf8_lossy(&output.stdout);
        for line in ports.lines() {
            if let Some(host_port) = line.split("->").next() {
                if let Some(port) = host_port.trim().split(':').last() {
                    let port = port.trim_end_matches("/tcp").trim_end_matches("/udp");
                    return Some(format!("http://localhost:{}", port));
                }
            }
        }
        None
    }

    fn start_searxng(&self) -> Result<(), String> {
        let url = format!("http://localhost:{}", SEARXNG_HOST_PORT);

        println!("Starting SearXNG on port {}...", SEARXNG_HOST_PORT);

        let output = Self::run_docker(&[
            "ps",
            "-a",
            "--filter",
            &format!("name={}", SEARXNG_CONTAINER_NAME),
            "--format",
            "{{.Names}}",
        ])?;

        let container_exists =
            String::from_utf8_lossy(&output.stdout).contains(SEARXNG_CONTAINER_NAME);

        if container_exists {
            Self::run_docker(&["start", SEARXNG_CONTAINER_NAME])?;
        } else {
            let secret = Command::new("openssl")
                .args(["rand", "-hex", "32"])
                .output()
                .map_err(|_| "openssl not found, using random")?;

            let secret_str = if secret.status.success() {
                String::from_utf8_lossy(&secret.stdout).trim().to_string()
            } else {
                "changeme".to_string()
            };

            Self::run_docker(&[
                "run",
                "-d",
                "--name",
                SEARXNG_CONTAINER_NAME,
                "-p",
                &format!("{}:{}", SEARXNG_HOST_PORT, SEARXNG_CONTAINER_PORT),
                "-e",
                &format!("SEARXNG_BASE_URL={}", url),
                "-e",
                &format!("SEARXNG_SECRET={}", secret_str),
                "-v",
                &format!("{}:/etc/searxng", SEARXNG_VOLUME_NAME),
                "searxng/searxng:latest",
            ])?;
        }

        self.ensure_container_running()?;
        self.enable_json_format()?;
        self.update_config_url(&url)?;
        Ok(())
    }

    fn is_connection_error(message: &str) -> bool {
        message.contains("Failed to connect")
            || message.contains("Connection refused")
            || message.contains("Empty reply")
            || message.contains("exit status: 7")
            || message.contains("exit status: 56")
    }

    fn run_docker(args: &[&str]) -> Result<Output, String> {
        let output = Command::new("docker")
            .args(args)
            .output()
            .map_err(|e| format!("docker {} failed: {}", args.get(0).unwrap_or(&""), e))?;

        if output.status.success() {
            return Ok(output);
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.trim();
        if detail.is_empty() {
            return Err(format!(
                "docker {} failed with status: {}",
                args.get(0).unwrap_or(&""),
                output.status
            ));
        }

        Err(format!(
            "docker {} failed: {}",
            args.get(0).unwrap_or(&""),
            detail
        ))
    }

    fn ensure_container_running(&self) -> Result<(), String> {
        let max_attempts = 5;
        for _ in 0..max_attempts {
            if let Some(status) = self.get_container_status()? {
                if status.starts_with("Up") {
                    return Ok(());
                }
                if status.starts_with("Exited") {
                    let logs = Self::run_docker(&[
                        "logs",
                        "--tail",
                        "50",
                        SEARXNG_CONTAINER_NAME,
                    ])
                    .ok()
                    .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                    .unwrap_or_default();

                    let hint = if logs.contains("limiter.toml is invalid") {
                        "Hint: invalid /etc/searxng/limiter.toml (old config). Remove the old config:\n  rm -rf ./searxng/config/\nthen retry."
                    } else {
                        "Check `docker logs searxng --tail 200` for details."
                    };

                    return Err(format!("SearXNG container exited. {}\n{}", hint, logs.trim()));
                }
            }

            std_sleep(Duration::from_secs(1));
        }

        Err("SearXNG container did not reach running state".to_string())
    }

    fn get_container_status(&self) -> Result<Option<String>, String> {
        let output = Self::run_docker(&[
            "ps",
            "-a",
            "--filter",
            &format!("name={}", SEARXNG_CONTAINER_NAME),
            "--format",
            "{{.Status}}",
        ])?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let status = stdout.lines().next().map(|s| s.trim());
        Ok(status.filter(|s| !s.is_empty()).map(|s| s.to_string()))
    }

    fn find_available_port(&self, start: u16) -> u16 {
        use std::net::TcpListener;
        for port in start..start + 100 {
            if TcpListener::bind(format!("127.0.0.1:{}", port)).is_ok() {
                return port;
            }
        }
        start
    }

    fn update_config_url(&self, url: &str) -> Result<(), String> {
        let config_path = std::env::var("HOME")
            .map(|h| format!("{}/.config/vibe_cli/.env", h))
            .unwrap_or_else(|_| ".env".to_string());

        let config_content = format!("SEARXNG_URL={}", url);

        if let Some(parent) = std::path::Path::new(&config_path).parent() {
            std::fs::create_dir_all(parent).ok();
        }

        std::fs::write(&config_path, config_content).ok();
        println!("Saved SEARXNG_URL={} to {}", url, config_path);
        Ok(())
    }

    fn enable_json_format(&self) -> Result<(), String> {
        let check_cmd = Self::run_docker(&[
            "exec",
            SEARXNG_CONTAINER_NAME,
            "grep",
            "-q",
            "-E",
            "^    - json$",
            "/etc/searxng/settings.yml",
        ]);

        if check_cmd.is_ok() && check_cmd.unwrap().status.success() {
            return Ok(());
        }

        Self::run_docker(&[
            "exec",
            SEARXNG_CONTAINER_NAME,
            "sh",
            "-c",
            "sed -i 's/^  formats:\\n    - html$/  formats:\\n    - html\\n    - json/' /etc/searxng/settings.yml || \
             sed -i '/^  formats:$/a\\    - html\\n    - json' /etc/searxng/settings.yml",
        ])?;

        Self::run_docker(&["restart", SEARXNG_CONTAINER_NAME])?;
        Ok(())
    }

    async fn searxng_search_with_url(
        &self,
        base_url: &str,
        query: &str,
        num_results: usize,
    ) -> Result<Vec<SearchResult>, String> {
        let encoded_query = url_encode(query);

        let url = format!(
            "{}/search?q={}&format=json&engines=general&language=en&safesearch=1&count={}",
            base_url, encoded_query, num_results
        );

        let output = Command::new("curl")
            .args([
                "-sS",
                "--connect-timeout",
                "10",
                "-m",
                "15",
                "--noproxy",
                "localhost,127.0.0.1,::1",
                "-A",
                "Mozilla/5.0 (compatible; vibe_cli/1.0; +https://github.com)",
                "-H",
                "Accept: application/json",
                "-w",
                "\n__CURL_HTTP_STATUS__:%{http_code}",
                &url,
            ])
            .output()
            .map_err(|e| format!("Search request failed: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let detail = stderr.trim();
            if detail.is_empty() {
                return Err(format!("curl failed with status: {}", output.status));
            }
            return Err(format!(
                "curl failed with status: {} ({})",
                output.status, detail
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let (body, http_status) = match stdout.rsplit_once("__CURL_HTTP_STATUS__:") {
            Some((body, status)) => (body.trim_end(), status.trim()),
            None => (stdout.as_ref(), ""),
        };

        if !http_status.is_empty() && http_status != "200" {
            let snippet: String = body.chars().take(200).collect();
            return Err(format!(
                "SearXNG returned HTTP {}: {}",
                http_status,
                snippet.replace('\n', " ")
            ));
        }

        #[derive(Deserialize)]
        struct SearxngResponse {
            results: Vec<SearxngResult>,
        }

        #[derive(Deserialize)]
        struct SearxngResult {
            title: String,
            url: String,
            content: Option<String>,
        }

        let response: SearxngResponse = serde_json::from_str(body).map_err(|e| {
            format!(
                "Parse error: {} - content: {}",
                e,
                &body[..body.len().min(200)]
            )
        })?;

        let results: Vec<SearchResult> = response
            .results
            .into_iter()
            .take(num_results)
            .map(|r| SearchResult {
                title: r.title,
                url: r.url,
                snippet: r.content.unwrap_or_default(),
            })
            .collect();

        Ok(results)
    }

    pub async fn fetch_page(&self, url: &str) -> Result<String, String> {
        let output = Command::new("curl")
            .args(["-sS", "--connect-timeout", "10", "-m", "15", "-L", url])
            .output()
            .map_err(|e| format!("Fetch failed: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let detail = stderr.trim();
            if detail.is_empty() {
                return Err(format!("Fetch failed with status: {}", output.status));
            }
            return Err(format!(
                "Fetch failed with status: {} ({})",
                output.status, detail
            ));
        }

        let text = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(text)
    }
}

fn url_encode(s: &str) -> String {
    let mut encoded = String::new();
    for c in s.chars() {
        match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => encoded.push(c),
            ' ' => encoded.push_str("%20"),
            _ => {
                for byte in c.to_string().as_bytes() {
                    encoded.push_str(&format!("%{:02X}", byte));
                }
            }
        }
    }
    encoded
}

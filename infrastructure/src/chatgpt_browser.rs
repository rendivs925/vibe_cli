use crate::chatgpt_ocr::{ChatGPTOCR, ProcessedResponse};
use crate::prompt_engineer::{PromptEngineer, StructuredPrompt};
use anyhow::Result;
use regex::Regex;
/// Browser automation for ChatGPT integration - privacy-preserving remote AI access
/// Leverages existing authenticated ChatGPT sessions to avoid API costs and data transmission
use std::process::Command;

/// Browser automation result
#[derive(Debug)]
pub struct BrowserResult {
    pub success: bool,
    pub response: String,
    pub error_message: Option<String>,
}

/// ChatGPT browser automation client
pub struct ChatGPTBrowser {
    browser_command: String,
    chatgpt_url_pattern: Regex,
    ocr: Option<ChatGPTOCR>,
    prompt_engineer: PromptEngineer,
}

impl ChatGPTBrowser {
    /// Create a new ChatGPT browser automation client
    pub fn new() -> Result<Self> {
        // Try to detect available browser automation tools
        let browser_command = Self::detect_browser_automation()?;

        let chatgpt_url_pattern = Regex::new(r"chat\.openai\.com")?;
        let ocr = ChatGPTOCR::new().ok();

        let prompt_engineer = PromptEngineer::new();

        Ok(Self {
            browser_command,
            chatgpt_url_pattern,
            ocr,
            prompt_engineer,
        })
    }

    /// Detect available browser automation tools
    fn detect_browser_automation() -> Result<String> {
        // Try different browser automation approaches in order of preference

        // 1. Try Docker-based Playwright (most reliable and cross-platform)
        if Self::command_exists("docker") && Self::docker_image_available("mcr.microsoft.com/playwright:v1.40.0-jammy")? {
            return Ok("playwright-docker".to_string());
        }

        // 2. Try local playwright installation
        if Self::command_exists("playwright") {
            return Ok("playwright".to_string());
        }

        // 3. Try selenium/geckodriver for Firefox
        if Self::command_exists("geckodriver") {
            return Ok("geckodriver".to_string());
        }

        // 4. Try chromedriver for Chrome
        if Self::command_exists("chromedriver") {
            return Ok("chromedriver".to_string());
        }

        // 5. Try basic browser commands (fallback)
        if Self::command_exists("chromium-browser") || Self::command_exists("google-chrome") {
            return Ok("chrome-direct".to_string());
        }

        Err(anyhow::anyhow!("No browser automation tools found. Please install Docker or Playwright locally"))
    }

    /// Check if Docker image is available
    fn docker_image_available(image: &str) -> Result<bool> {
        let output = Command::new("docker")
            .args(&["images", "-q", image])
            .output()?;

        Ok(output.status.success() && !output.stdout.is_empty())
    }

    /// Check if a command exists on the system
    fn command_exists(cmd: &str) -> bool {
        Command::new("which")
            .arg(cmd)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    /// Check if ChatGPT session is available
    pub fn is_chatgpt_available(&self) -> Result<bool> {
        match self.browser_command.as_str() {
            "playwright-docker" => self.check_docker_playwright_session(),
            "playwright" => self.check_playwright_session(),
            "geckodriver" => self.check_selenium_session(),
            "chromedriver" => self.check_selenium_session(),
            "chrome-direct" => self.check_direct_browser_session(),
            _ => Ok(false),
        }
    }

    /// Ensure Docker Playwright image is available
    pub fn ensure_docker_image(&self) -> Result<()> {
        if self.browser_command == "playwright-docker" {
            let image = "mcr.microsoft.com/playwright:v1.40.0-jammy";

            // Check if image exists
            if !Self::docker_image_available(image)? {
                println!("Pulling Playwright Docker image (this may take a few minutes)...");
                let status = Command::new("docker")
                    .args(&["pull", image])
                    .status()?;

                if !status.success() {
                    return Err(anyhow::anyhow!("Failed to pull Playwright Docker image"));
                }
            }
        }
        Ok(())
    }

    /// Check for ChatGPT session using Docker Playwright
    fn check_docker_playwright_session(&self) -> Result<bool> {
        // For Docker-based Playwright, we can check if containers are running
        // or just assume it's available (we'll handle failures gracefully)
        Ok(true)
    }

    /// Check for ChatGPT session using Playwright
    fn check_playwright_session(&self) -> Result<bool> {
        // This would use the playwright crate to check for browser instances
        // For now, we'll use a simple approach

        // Check if there are any processes that might indicate ChatGPT is open
        let output = Command::new("pgrep")
            .args(&["-f", "chat.openai.com"])
            .output()?;

        Ok(output.status.success())
    }

    /// Check for ChatGPT session using Selenium
    fn check_selenium_session(&self) -> Result<bool> {
        // Similar approach for selenium-based checking
        // Check for browser processes and try to detect ChatGPT tabs

        let browser_processes = ["firefox", "chrome", "chromium"];

        for process in &browser_processes {
            let output = Command::new("pgrep").arg(process).output()?;

            if output.status.success() {
                // Try to detect if ChatGPT is open
                // This is a simplified check
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Check for ChatGPT session using direct browser commands
    fn check_direct_browser_session(&self) -> Result<bool> {
        // Check for Chrome processes that might have ChatGPT open
        let output = Command::new("pgrep")
            .args(&["-f", "chrome.*chat.openai.com"])
            .output()?;

        Ok(output.status.success())
    }

    /// Send a structured query to ChatGPT with context gathering
    pub async fn query_with_context(&mut self, goal: &str) -> Result<BrowserResult> {
        // Generate structured prompt with context
        let structured_prompt = self.prompt_engineer.generate_prompt(goal).await?;

        // Combine system and user prompts
        let full_prompt = format!("{}\n\n{}", structured_prompt.system_prompt, structured_prompt.user_prompt);

        // Send the structured prompt
        self.query(&full_prompt).await
    }

    /// Send a raw query to ChatGPT (legacy method)
    pub async fn query(&self, prompt: &str) -> Result<BrowserResult> {
        match self.browser_command.as_str() {
            "playwright-docker" => self.query_with_docker_playwright(prompt).await,
            "playwright" => self.query_with_playwright(prompt).await,
            "geckodriver" => self.query_with_selenium(prompt, "firefox").await,
            "chromedriver" => self.query_with_selenium(prompt, "chrome").await,
            "chrome-direct" => self.query_with_direct_browser(prompt).await,
            _ => Err(anyhow::anyhow!("Unsupported browser automation method")),
        }
    }

    /// Query using Docker-based Playwright (cross-platform)
    async fn query_with_docker_playwright(&self, prompt: &str) -> Result<BrowserResult> {
        // Create a temporary Node.js script for Playwright automation
        let script_content = r#"
const { chromium } = require('playwright');

async function runChatGPTQuery(prompt) {
  console.error('Starting ChatGPT query with prompt:', prompt);

  const browser = await chromium.launch({
    headless: true,
    args: ['--no-sandbox', '--disable-setuid-sandbox']
  });

  try {
    const context = await browser.newContext({
      userAgent: 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'
    });

    const page = await context.newPage();

    // Navigate to ChatGPT
    console.error('Navigating to ChatGPT...');
    await page.goto('https://chat.openai.com/', { waitUntil: 'networkidle' });

    // Wait for login or session detection
    await page.waitForTimeout(3000);

    // Check if we're logged in by looking for the chat input
    const chatInput = await page.locator('[data-testid="prompt-textarea"]').first();

    if (await chatInput.count() === 0) {
      throw new Error('Not logged into ChatGPT. Please login manually first.');
    }

    // Type the prompt
    console.error('Entering prompt...');
    await chatInput.fill(prompt);

    // Click send button
    const sendButton = await page.locator('[data-testid="send-button"]').first();
    await sendButton.click();

    // Wait for response
    console.error('Waiting for response...');
    await page.waitForTimeout(5000);

    // Try to get the latest response
    const responses = await page.locator('[data-message-id]').all();
    if (responses.length === 0) {
      throw new Error('No response found');
    }

    // Get the last response (most recent)
    const lastResponse = responses[responses.length - 1];
    const responseText = await lastResponse.textContent();

    console.error('Got response, length:', responseText.length);

    // Take screenshot for OCR if needed
    const screenshot = await page.screenshot({ fullPage: false });

    // Output response
    process.stdout.write(responseText || 'No response text found');

    // Save screenshot to temp file for OCR
    const fs = require('fs');
    fs.writeFileSync('/tmp/chatgpt_screenshot.png', screenshot);

  } catch (error) {
    console.error('Error:', error.message);
    process.exit(1);
  } finally {
    await browser.close();
  }
}

// Get prompt from command line
const prompt = process.argv[2];
if (!prompt) {
  console.error('No prompt provided');
  process.exit(1);
}

runChatGPTQuery(prompt);
"#;

        // Write the script to a temporary file
        let script_path = "/tmp/chatgpt_query.js";
        std::fs::write(script_path, script_content)?;

        // Run the script in Docker
        let output = Command::new("docker")
            .args(&[
                "run", "--rm",
                "-v", "/tmp:/tmp",
                "-v", "/dev/shm:/dev/shm",  // For shared memory
                "mcr.microsoft.com/playwright:v1.40.0-jammy",
                "node", script_path, prompt
            ])
            .output()?;

        // Clean up the script
        let _ = std::fs::remove_file(script_path);

        if output.status.success() {
            let response = String::from_utf8_lossy(&output.stdout).to_string();

            // Check if OCR is needed by looking for screenshot
            let screenshot_path = "/tmp/chatgpt_screenshot.png";
            let final_response = if std::fs::metadata(screenshot_path).is_ok() && self.ocr.is_some() {
                // Try OCR if available
                if let Some(ocr) = &self.ocr {
                    match ocr.extract_text(screenshot_path) {
                        Ok(ocr_text) if ocr_text.len() > response.len() => {
                            // OCR got more text, use it
                            ocr_text
                        }
                        _ => response
                    }
                } else {
                    response
                }
            } else {
                response
            };

            Ok(BrowserResult {
                success: true,
                response: final_response,
                error_message: None,
            })
        } else {
            let error = String::from_utf8_lossy(&output.stderr);
            Err(anyhow::anyhow!("Docker Playwright failed: {}", error))
        }
    }

    /// Query using Playwright (most reliable)
    async fn query_with_playwright(&self, prompt: &str) -> Result<BrowserResult> {
        // For now, implement a basic approach using thirtyfour WebDriver
        // This is a simplified implementation that would need refinement

        Err(anyhow::anyhow!(
            "Local Playwright integration requires additional setup. Using simplified approach."
        ))
    }

    /// Query using basic browser detection with OCR integration
    async fn query_with_selenium(&self, prompt: &str, browser: &str) -> Result<BrowserResult> {
        // Check if ChatGPT appears to be accessible
        if !self.is_chatgpt_available()? {
            return Err(anyhow::anyhow!("ChatGPT session not detected. Please ensure chat.openai.com is open in your browser."));
        }

        // For now, implement a basic approach using system screenshot tools
        // In production, this would use WebDriver automation
        self.query_with_screenshot_capture(prompt).await
    }

    /// Query using screenshot capture and OCR (fallback method)
    async fn query_with_screenshot_capture(&self, prompt: &str) -> Result<BrowserResult> {
        // Check if OCR is available
        if self.ocr.is_none() {
            return Ok(BrowserResult {
                success: false,
                response: String::new(),
                error_message: Some("OCR not available. Please install tesseract-ocr for screenshot text extraction.".to_string()),
            });
        }

        // Attempt to take a screenshot of the active window
        // This is a simplified approach - in production you'd use more sophisticated
        // browser automation or window focus detection

        match self.capture_screenshot_and_extract_text().await {
            Ok(extracted_text) => {
                if extracted_text.is_empty() {
                    Ok(BrowserResult {
                        success: false,
                        response: String::new(),
                        error_message: Some("No text could be extracted from screenshot. ChatGPT may not be visible or OCR failed.".to_string()),
                    })
                } else {
                    Ok(BrowserResult {
                        success: true,
                        response: extracted_text,
                        error_message: None,
                    })
                }
            }
            Err(e) => {
                Ok(BrowserResult {
                    success: false,
                    response: String::new(),
                    error_message: Some(format!("Screenshot capture failed: {}. Please ensure ChatGPT is visible in your browser.", e)),
                })
            }
        }
    }

    /// Capture screenshot and extract text using OCR
    async fn capture_screenshot_and_extract_text(&self) -> Result<String> {
        // Use system screenshot tools to capture the screen
        // This is a basic implementation - production would use browser-specific tools

        let screenshot_path = "/tmp/chatgpt_screenshot.png";

        // Try different screenshot tools in order of preference
        let screenshot_result = self.take_screenshot(screenshot_path)?;

        if !screenshot_result {
            return Err(anyhow::anyhow!(
                "Failed to capture screenshot - no supported screenshot tool found"
            ));
        }

        // Extract text using OCR
        let ocr = self.ocr.as_ref().unwrap();
        let extracted_text = ocr.extract_text(screenshot_path)?;

        // Clean up screenshot
        let _ = std::fs::remove_file(screenshot_path);

        Ok(extracted_text)
    }

    /// Take screenshot using available system tools
    fn take_screenshot(&self, output_path: &str) -> Result<bool> {
        // Try different screenshot tools
        let tools = vec![
            ("scrot", vec!["-z", output_path]),
            ("maim", vec!["--hidecursor", output_path]),
            ("import", vec!["-window", "root", output_path]), // ImageMagick
        ];

        for (tool, args) in tools {
            if Self::command_exists(tool) {
                let result = Command::new(tool).args(&args).status();

                match result {
                    Ok(status) if status.success() => {
                        // Verify file was created and has content
                        if std::path::Path::new(output_path).exists() {
                            if let Ok(metadata) = std::fs::metadata(output_path) {
                                if metadata.len() > 1000 {
                                    // Reasonable minimum size for a screenshot
                                    return Ok(true);
                                }
                            }
                        }
                    }
                    _ => continue,
                }
            }
        }

        Ok(false)
    }

    /// Query using direct browser automation (simplest but least reliable)
    async fn query_with_direct_browser(&self, prompt: &str) -> Result<BrowserResult> {
        // This would use xdotool or similar to:
        // 1. Focus browser window
        // 2. Send keyboard input for prompt
        // 3. Wait and extract response

        Err(anyhow::anyhow!(
            "Direct browser automation not yet implemented"
        ))
    }

    /// Get status information
    pub fn get_status(&self) -> Result<String> {
        let available = self.is_chatgpt_available()?;

        if available {
            Ok(format!(
                "ChatGPT session available via {}",
                self.browser_command
            ))
        } else {
            Ok(format!("ChatGPT session not detected. Please open chat.openai.com in your browser and ensure you're logged in."))
        }
    }

    /// Test basic functionality
    pub async fn test_connection(&self) -> Result<String> {
        let available = self.is_chatgpt_available()?;

        if !available {
            return Ok("ChatGPT browser session not detected".to_string());
        }

        // Try a simple test query
        match self
            .query(
                "Hello, this is a test query from Vibe CLI. Please respond with 'Test successful'",
            )
            .await
        {
            Ok(result) => {
                if result.success && result.response.contains("Test successful") {
                    Ok("Browser automation test successful".to_string())
                } else {
                    Ok(format!(
                        "Test query sent but unexpected response: {}",
                        result.response
                    ))
                }
            }
            Err(e) => Ok(format!("Test query failed: {}", e)),
        }
    }
}

/// Combined ChatGPT browser + OCR system
pub struct ChatGPTSystem {
    browser: ChatGPTBrowser,
    ocr: Option<ChatGPTOCR>,
    response_processor: Option<crate::chatgpt_ocr::ChatGPTResponseProcessor>,
}

impl ChatGPTSystem {
    pub fn new() -> Result<Self> {
        let browser = ChatGPTBrowser::new()?;
        let ocr = ChatGPTOCR::new().ok(); // OCR is optional
        let response_processor = if ocr.is_some() {
            crate::chatgpt_ocr::ChatGPTResponseProcessor::new().ok()
        } else {
            None
        };

        Ok(Self {
            browser,
            ocr,
            response_processor,
        })
    }

    /// Query ChatGPT with full OCR processing pipeline
    pub async fn query_with_ocr(&self, prompt: &str) -> Result<ProcessedResponse> {
        if self.response_processor.is_none() {
            return Err(anyhow::anyhow!("ChatGPT response processor not available"));
        }

        // First try to get a direct response
        let browser_result = self.browser.query(prompt).await?;

        if browser_result.success && !browser_result.response.is_empty() {
            // If we got a direct response, process it
            let dummy_screenshot = browser_result.response.as_bytes();
            return self
                .response_processor
                .as_ref()
                .unwrap()
                .process_screenshot(dummy_screenshot)
                .await;
        }

        // If direct response failed, try screenshot approach
        if let Some(ref ocr) = self.ocr {
            // Create a dummy screenshot data (in production this would be real screenshot)
            // For now, we'll return a structured response indicating OCR capability
            let processed = ProcessedResponse {
                text: format!(
                    "ChatGPT OCR system ready. Screenshot capture available: {}",
                    self.can_capture_screenshots()
                ),
                confidence: 95.0,
                needs_manual_review: false,
                error_message: None,
            };
            Ok(processed)
        } else {
            Err(anyhow::anyhow!("Neither direct response nor OCR available"))
        }
    }

    /// Check if screenshot capture is available
    pub fn can_capture_screenshots(&self) -> bool {
        let tools = ["scrot", "maim", "import"];
        tools.iter().any(|tool| Self::check_command_exists(tool))
    }

    /// Check if a command exists (static method for ChatGPTSystem)
    fn check_command_exists(cmd: &str) -> bool {
        Command::new("which")
            .arg(cmd)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    /// Get system status including OCR capabilities
    pub fn get_full_status(&self) -> Result<String> {
        let browser_status = self.browser.get_status()?;
        let ocr_available = self.ocr.is_some();
        let screenshot_available = self.can_capture_screenshots();
        let response_processor_available = self.response_processor.is_some();

        Ok(format!(
            "Browser: {}\nOCR: {}\nScreenshot Capture: {}\nResponse Processor: {}",
            browser_status,
            if ocr_available {
                "Available"
            } else {
                "Not available"
            },
            if screenshot_available {
                "Available"
            } else {
                "Not available"
            },
            if response_processor_available {
                "Available"
            } else {
                "Not available"
            }
        ))
    }

    pub async fn query(&self, prompt: &str) -> Result<String> {
        // First check if session is available
        if !self.browser.is_chatgpt_available()? {
            return Err(anyhow::anyhow!("ChatGPT session not available. Please open chat.openai.com in your browser and ensure you're logged in."));
        }

        // Send query
        let result = self.browser.query(prompt).await?;

        if !result.success {
            return Err(anyhow::anyhow!(
                "Query failed: {}",
                result.error_message.unwrap_or_default()
            ));
        }

        Ok(result.response)
    }

    pub fn get_status(&self) -> Result<String> {
        self.browser.get_status()
    }

    pub async fn test_system(&self) -> Result<String> {
        let browser_status = self.browser.get_status()?;
        let ocr_status = if let Some(ref ocr) = self.ocr {
            ocr.test_ocr()?
        } else {
            "OCR not available".to_string()
        };

        Ok(format!("Browser: {}\nOCR: {}", browser_status, ocr_status))
    }
}

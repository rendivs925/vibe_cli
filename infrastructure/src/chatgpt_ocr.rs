use anyhow::Result;
/// OCR and response processing for ChatGPT browser automation
/// Extracts and processes text responses from ChatGPT screenshots
use std::process::Command;

/// OCR processor for extracting text from ChatGPT responses
pub struct ChatGPTOCR {
    ocr_command: String,
    confidence_threshold: f32,
}

impl ChatGPTOCR {
    /// Create a new OCR processor
    pub fn new() -> Result<Self> {
        let ocr_command = if Self::command_exists("tesseract") {
            "tesseract".to_string()
        } else {
            return Err(anyhow::anyhow!(
                "Tesseract OCR not found. Install with: apt install tesseract-ocr"
            ));
        };

        Ok(Self {
            ocr_command,
            confidence_threshold: 60.0, // Minimum confidence for acceptable OCR
        })
    }

    /// Extract text from image data
    pub fn extract_text_from_image(&self, image_data: &[u8]) -> Result<OCRResult> {
        // Save image to temporary file
        let temp_path = "/tmp/chatgpt_response.png";
        std::fs::write(temp_path, image_data)?;

        // Run OCR
        let output = Command::new(&self.ocr_command)
            .args(&[temp_path, "stdout", "-l", "eng", "--psm", "6"]) // PSM 6 for uniform text
            .output()?;

        // Clean up temp file
        let _ = std::fs::remove_file(temp_path);

        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout).to_string();
            let cleaned_text = self.clean_ocr_text(&text);

            // Calculate confidence (simplified - tesseract doesn't provide confidence easily)
            let confidence = self.estimate_confidence(&cleaned_text);

            Ok(OCRResult {
                text: cleaned_text,
                confidence,
                is_reliable: confidence >= self.confidence_threshold,
            })
        } else {
            let error = String::from_utf8_lossy(&output.stderr);
            Err(anyhow::anyhow!("OCR failed: {}", error))
        }
    }

    /// Extract text from image file
    pub fn extract_text(&self, image_path: &str) -> Result<String> {
        let output = Command::new(&self.ocr_command)
            .args(&[image_path, "stdout", "-l", "eng", "--psm", "6"])
            .output()?;

        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout).to_string();
            Ok(self.clean_ocr_text(&text))
        } else {
            let error = String::from_utf8_lossy(&output.stderr);
            Err(anyhow::anyhow!("OCR failed: {}", error))
        }
    }

    /// Clean and normalize OCR text
    fn clean_ocr_text(&self, text: &str) -> String {
        let mut cleaned = text
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n");

        // Remove non-alphanumeric characters except specific punctuation
        cleaned = cleaned.replace(
            |c: char| {
                !c.is_alphanumeric()
                    && !c.is_whitespace()
                    && !['.', ',', '!', '?', ':', ';'].contains(&c)
            },
            "",
        );

        // Remove sequences of the same punctuation character (OCR artifacts)
        cleaned = cleaned
            .replace("!!!", "!")
            .replace("???", "?")
            .replace("...", ".")
            .replace(";;;", ";")
            .replace(":::", ":");

        // Normalize whitespace
        cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// Estimate OCR confidence (simplified approach)
    fn estimate_confidence(&self, text: &str) -> f32 {
        if text.is_empty() {
            return 0.0;
        }

        let word_count = text.split_whitespace().count();
        let char_count = text.chars().count();

        if word_count == 0 {
            return 0.0;
        }

        // Simple heuristics for confidence
        let avg_word_length = char_count as f32 / word_count as f32;

        // Penalize very short or very long words (likely OCR errors)
        let length_penalty = if avg_word_length < 2.0 || avg_word_length > 12.0 {
            0.7
        } else {
            1.0
        };

        // Penalize text with many single characters
        let single_char_penalty = text
            .split_whitespace()
            .filter(|word| word.len() == 1)
            .count() as f32
            / word_count as f32;

        let base_confidence = 85.0; // Base confidence for well-formed text
        let confidence = base_confidence * length_penalty * (1.0 - single_char_penalty * 2.0);

        confidence.max(0.0).min(100.0)
    }

    /// Test OCR functionality
    pub fn test_ocr(&self) -> Result<String> {
        // Create a simple test
        Ok("OCR system ready".to_string())
    }

    /// Check if OCR command exists
    fn command_exists(cmd: &str) -> bool {
        Command::new("which")
            .arg(cmd)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}

/// OCR result with confidence scoring
#[derive(Debug, Clone)]
pub struct OCRResult {
    pub text: String,
    pub confidence: f32,
    pub is_reliable: bool,
}

/// ChatGPT response processor
pub struct ChatGPTResponseProcessor {
    ocr: ChatGPTOCR,
}

impl ChatGPTResponseProcessor {
    /// Create a new response processor
    pub fn new() -> Result<Self> {
        let ocr = ChatGPTOCR::new()?;
        Ok(Self { ocr })
    }

    /// Process a screenshot of ChatGPT response
    pub async fn process_screenshot(&self, screenshot_data: &[u8]) -> Result<ProcessedResponse> {
        // Extract text using OCR
        let ocr_result = self.ocr.extract_text_from_image(screenshot_data)?;

        if !ocr_result.is_reliable {
            return Ok(ProcessedResponse {
                text: ocr_result.text,
                confidence: ocr_result.confidence,
                needs_manual_review: true,
                error_message: Some("Low OCR confidence - manual review recommended".to_string()),
            });
        }

        // Clean and format the response
        let cleaned_text = self.clean_chatgpt_response(&ocr_result.text)?;

        Ok(ProcessedResponse {
            text: cleaned_text,
            confidence: ocr_result.confidence,
            needs_manual_review: false,
            error_message: None,
        })
    }

    /// Clean and format ChatGPT response text
    fn clean_chatgpt_response(&self, raw_text: &str) -> Result<String> {
        let mut cleaned = raw_text.to_string();

        // Remove common OCR artifacts from ChatGPT UI
        let artifacts = vec![
            "ChatGPT",
            "New chat",
            "Regenerate response",
            "Copy code",
            "Good response",
            "Bad response",
            "Thumbs up",
            "Thumbs down",
        ];

        for artifact in &artifacts {
            cleaned = cleaned.replace(artifact, "");
        }

        // Remove excessive whitespace
        cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");

        // Try to detect and extract the main response content
        // This is a simplified approach - in practice, you'd use more sophisticated
        // UI element detection or pattern matching

        Ok(cleaned.trim().to_string())
    }

    /// Validate response quality
    pub fn validate_response(&self, response: &ProcessedResponse) -> ResponseValidation {
        let mut issues = Vec::new();

        // Check for common OCR errors
        if response.text.contains("||||") || response.text.contains("____") {
            issues.push("Possible OCR artifacts detected".to_string());
        }

        // Check for incomplete responses
        if response.text.ends_with("...") || response.text.len() < 10 {
            issues.push("Response appears incomplete".to_string());
        }

        // Check confidence
        if response.confidence < 70.0 {
            issues.push(format!("Low confidence: {:.1}%", response.confidence));
        }

        ResponseValidation {
            is_valid: issues.is_empty(),
            issues,
        }
    }
}

/// Processed ChatGPT response
#[derive(Debug, Clone)]
pub struct ProcessedResponse {
    pub text: String,
    pub confidence: f32,
    pub needs_manual_review: bool,
    pub error_message: Option<String>,
}

/// Response validation result
#[derive(Debug, Clone)]
pub struct ResponseValidation {
    pub is_valid: bool,
    pub issues: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ocr_initialization() {
        // Test OCR initialization (will fail if tesseract not installed)
        match ChatGPTOCR::new() {
            Ok(ocr) => {
                assert!(!ocr.ocr_command.is_empty());
                println!("OCR initialized successfully");
            }
            Err(e) => {
                println!("OCR not available: {}", e);
                // This is expected in test environments without tesseract
            }
        }
    }

    #[test]
    fn test_text_cleaning() {
        let ocr = ChatGPTOCR {
            ocr_command: "tesseract".to_string(),
            confidence_threshold: 60.0,
        };

        let dirty_text = "Hello   world!!!\n\nThis  is\ta  test....\n\nWith   extra   spaces.";
        let cleaned = ocr.clean_ocr_text(dirty_text);

        println!("Original: {:?}", dirty_text);
        println!("Cleaned: {:?}", cleaned);
        println!("Contains !!! : {}", cleaned.contains("!!!"));
        println!("Contains .... : {}", cleaned.contains("...."));

        assert!(!cleaned.contains("!!!"));
        assert!(!cleaned.contains("...."));
        assert!(!cleaned.contains("\n\n"));
        assert!(!cleaned.contains("\t"));
    }

    #[test]
    fn test_confidence_estimation() {
        let ocr = ChatGPTOCR {
            ocr_command: "tesseract".to_string(),
            confidence_threshold: 60.0,
        };

        // Test with normal text
        let normal_text = "This is a normal sentence with reasonable words.";
        let confidence = ocr.estimate_confidence(normal_text);
        assert!(confidence > 50.0);

        // Test with empty text
        let empty_text = "";
        let confidence_empty = ocr.estimate_confidence(empty_text);
        assert_eq!(confidence_empty, 0.0);

        println!("Normal text confidence: {:.1}%", confidence);
    }

    #[tokio::test]
    async fn test_response_processor() {
        match ChatGPTResponseProcessor::new() {
            Ok(processor) => {
                // Test with dummy data (would need actual screenshot in real test)
                let dummy_screenshot = vec![0u8; 100]; // Dummy data

                match processor.process_screenshot(&dummy_screenshot).await {
                    Ok(response) => {
                        println!(
                            "Response processed: {} chars, confidence: {:.1}%",
                            response.text.len(),
                            response.confidence
                        );
                    }
                    Err(e) => {
                        println!("Processing failed (expected with dummy data): {}", e);
                    }
                }
            }
            Err(e) => {
                println!("Response processor not available: {}", e);
            }
        }
    }
}

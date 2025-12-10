//! LLM client for Japanese text proofreading suggestions
//!
//! Supports Claude (Anthropic) and OpenAI APIs.

use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::config::Config;

/// LLM client for making API requests
pub struct LlmClient {
    client: Client,
    config: Config,
}

/// Request for proofreading suggestions
#[derive(Debug, Clone)]
pub struct ProofreadRequest {
    /// Original text to proofread
    pub text: String,
    /// Context around the text (optional)
    pub context: Option<String>,
    /// Specific issue to address (optional)
    pub issue: Option<String>,
}

/// Response from proofreading
#[derive(Debug, Clone)]
pub struct ProofreadResponse {
    /// Suggested correction
    pub suggestion: String,
    /// Explanation of the correction
    pub explanation: String,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,
}

// Claude API types
#[derive(Serialize)]
struct ClaudeRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<ClaudeMessage>,
}

#[derive(Serialize)]
struct ClaudeMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ClaudeResponse {
    content: Vec<ClaudeContent>,
}

#[derive(Deserialize)]
struct ClaudeContent {
    text: String,
}

// OpenAI API types
#[derive(Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    max_tokens: u32,
}

#[derive(Serialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessageResponse,
}

#[derive(Deserialize)]
struct OpenAiMessageResponse {
    content: String,
}

// Parsed suggestion from LLM response
#[derive(Deserialize)]
struct ParsedSuggestion {
    suggestion: String,
    explanation: String,
    confidence: f32,
}

impl LlmClient {
    /// Create a new LLM client with the given configuration
    pub fn new(config: Config) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    /// Check if LLM integration is available
    pub fn is_available(&self) -> bool {
        self.config.is_llm_enabled()
    }

    /// Get proofreading suggestion for the given text
    pub async fn proofread(&self, request: ProofreadRequest) -> Result<ProofreadResponse> {
        if !self.is_available() {
            return Err(anyhow!("LLM integration is not configured"));
        }

        let prompt = self.build_prompt(&request);
        let response = match self.config.llm.provider.as_str() {
            "claude" => self.call_claude(&prompt).await?,
            "openai" => self.call_openai(&prompt).await?,
            _ => return Err(anyhow!("Unknown LLM provider: {}", self.config.llm.provider)),
        };

        self.parse_response(&response)
    }

    /// Build the prompt for proofreading
    fn build_prompt(&self, request: &ProofreadRequest) -> String {
        let mut prompt = String::from(
            "あなたは日本語校正の専門家です。以下のテキストを校正し、修正案を提示してください。\n\n",
        );

        if let Some(ref context) = request.context {
            prompt.push_str(&format!("【文脈】\n{}\n\n", context));
        }

        prompt.push_str(&format!("【校正対象テキスト】\n{}\n\n", request.text));

        if let Some(ref issue) = request.issue {
            prompt.push_str(&format!("【検出された問題】\n{}\n\n", issue));
        }

        prompt.push_str(
            r#"以下のJSON形式で回答してください：
{
  "suggestion": "修正後のテキスト",
  "explanation": "修正理由の説明",
  "confidence": 0.0〜1.0の確信度
}

JSONのみを出力し、それ以外のテキストは含めないでください。"#,
        );

        prompt
    }

    /// Call Claude API
    async fn call_claude(&self, prompt: &str) -> Result<String> {
        let api_key = self
            .config
            .get_api_key()
            .ok_or_else(|| anyhow!("Claude API key not found"))?;

        let base_url = self
            .config
            .llm
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.anthropic.com".to_string());

        let request = ClaudeRequest {
            model: self.config.get_model(),
            max_tokens: self.config.llm.max_tokens,
            messages: vec![ClaudeMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
        };

        let response = self
            .client
            .post(format!("{}/v1/messages", base_url))
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Claude API error: {} - {}", status, body));
        }

        let claude_response: ClaudeResponse = response.json().await?;
        claude_response
            .content
            .first()
            .map(|c| c.text.clone())
            .ok_or_else(|| anyhow!("Empty response from Claude"))
    }

    /// Call OpenAI API
    async fn call_openai(&self, prompt: &str) -> Result<String> {
        let api_key = self
            .config
            .get_api_key()
            .ok_or_else(|| anyhow!("OpenAI API key not found"))?;

        let base_url = self
            .config
            .llm
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.openai.com".to_string());

        let request = OpenAiRequest {
            model: self.config.get_model(),
            max_tokens: self.config.llm.max_tokens,
            messages: vec![OpenAiMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
        };

        let response = self
            .client
            .post(format!("{}/v1/chat/completions", base_url))
            .header("Authorization", format!("Bearer {}", api_key))
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("OpenAI API error: {} - {}", status, body));
        }

        let openai_response: OpenAiResponse = response.json().await?;
        openai_response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| anyhow!("Empty response from OpenAI"))
    }

    /// Parse the LLM response into a ProofreadResponse
    fn parse_response(&self, response: &str) -> Result<ProofreadResponse> {
        // Try to extract JSON from the response
        let json_str = self.extract_json(response)?;

        let parsed: ParsedSuggestion = serde_json::from_str(&json_str)
            .map_err(|e| anyhow!("Failed to parse LLM response: {} - Response: {}", e, json_str))?;

        Ok(ProofreadResponse {
            suggestion: parsed.suggestion,
            explanation: parsed.explanation,
            confidence: parsed.confidence.clamp(0.0, 1.0),
        })
    }

    /// Extract JSON from potentially wrapped response
    fn extract_json(&self, response: &str) -> Result<String> {
        let trimmed = response.trim();

        // If it starts with {, assume it's JSON
        if trimmed.starts_with('{') {
            // Find the matching closing brace
            let mut depth = 0;
            let mut end_idx = 0;
            for (i, c) in trimmed.char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            end_idx = i + 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if end_idx > 0 {
                return Ok(trimmed[..end_idx].to_string());
            }
        }

        // Try to find JSON in code blocks
        if let Some(start) = trimmed.find("```json") {
            let json_start = start + 7;
            if let Some(end) = trimmed[json_start..].find("```") {
                return Ok(trimmed[json_start..json_start + end].trim().to_string());
            }
        }

        // Try to find any JSON object
        if let Some(start) = trimmed.find('{') {
            if let Some(end) = trimmed.rfind('}') {
                return Ok(trimmed[start..=end].to_string());
            }
        }

        Err(anyhow!("Could not extract JSON from response: {}", response))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LlmConfig;

    fn create_test_config(provider: &str) -> Config {
        Config {
            llm: LlmConfig {
                provider: provider.to_string(),
                api_key: Some("test-key".to_string()),
                model: None,
                base_url: None,
                max_tokens: 1024,
            },
            ..Default::default()
        }
    }

    #[test]
    fn test_client_creation() {
        let config = create_test_config("claude");
        let client = LlmClient::new(config);
        assert!(client.is_available());
    }

    #[test]
    fn test_client_not_available_when_disabled() {
        let config = Config::default(); // provider = "none"
        let client = LlmClient::new(config);
        assert!(!client.is_available());
    }

    #[test]
    fn test_build_prompt_simple() {
        let config = create_test_config("claude");
        let client = LlmClient::new(config);

        let request = ProofreadRequest {
            text: "テスト文章".to_string(),
            context: None,
            issue: None,
        };

        let prompt = client.build_prompt(&request);
        assert!(prompt.contains("テスト文章"));
        assert!(prompt.contains("校正対象テキスト"));
    }

    #[test]
    fn test_build_prompt_with_context() {
        let config = create_test_config("claude");
        let client = LlmClient::new(config);

        let request = ProofreadRequest {
            text: "テスト文章".to_string(),
            context: Some("前の文章".to_string()),
            issue: Some("ら抜き言葉".to_string()),
        };

        let prompt = client.build_prompt(&request);
        assert!(prompt.contains("テスト文章"));
        assert!(prompt.contains("前の文章"));
        assert!(prompt.contains("ら抜き言葉"));
    }

    #[test]
    fn test_extract_json_direct() {
        let config = create_test_config("claude");
        let client = LlmClient::new(config);

        let response = r#"{"suggestion": "test", "explanation": "reason", "confidence": 0.9}"#;
        let json = client.extract_json(response).unwrap();
        assert!(json.contains("suggestion"));
    }

    #[test]
    fn test_extract_json_from_code_block() {
        let config = create_test_config("claude");
        let client = LlmClient::new(config);

        let response = r#"Here is the result:
```json
{"suggestion": "test", "explanation": "reason", "confidence": 0.9}
```"#;
        let json = client.extract_json(response).unwrap();
        assert!(json.contains("suggestion"));
    }

    #[test]
    fn test_extract_json_with_surrounding_text() {
        let config = create_test_config("claude");
        let client = LlmClient::new(config);

        let response = r#"I will fix this for you:
{"suggestion": "fixed text", "explanation": "grammar fix", "confidence": 0.85}
Hope this helps!"#;
        let json = client.extract_json(response).unwrap();
        assert!(json.contains("fixed text"));
    }

    #[test]
    fn test_parse_response_valid() {
        let config = create_test_config("claude");
        let client = LlmClient::new(config);

        let response = r#"{"suggestion": "修正後", "explanation": "理由", "confidence": 0.9}"#;
        let result = client.parse_response(response).unwrap();

        assert_eq!(result.suggestion, "修正後");
        assert_eq!(result.explanation, "理由");
        assert!((result.confidence - 0.9).abs() < 0.01);
    }

    #[test]
    fn test_parse_response_clamps_confidence() {
        let config = create_test_config("claude");
        let client = LlmClient::new(config);

        let response = r#"{"suggestion": "test", "explanation": "test", "confidence": 1.5}"#;
        let result = client.parse_response(response).unwrap();
        assert_eq!(result.confidence, 1.0);

        let response = r#"{"suggestion": "test", "explanation": "test", "confidence": -0.5}"#;
        let result = client.parse_response(response).unwrap();
        assert_eq!(result.confidence, 0.0);
    }

    #[test]
    fn test_parse_response_invalid_json() {
        let config = create_test_config("claude");
        let client = LlmClient::new(config);

        let response = "not json at all";
        assert!(client.parse_response(response).is_err());
    }

    #[test]
    fn test_proofread_request_creation() {
        let request = ProofreadRequest {
            text: "食べれる".to_string(),
            context: Some("彼は魚を".to_string()),
            issue: Some("ら抜き言葉の可能性".to_string()),
        };

        assert_eq!(request.text, "食べれる");
        assert_eq!(request.context, Some("彼は魚を".to_string()));
    }
}

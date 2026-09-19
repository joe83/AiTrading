use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::{debug, error, info};

use crate::config::GrokConfig;

/// Client for the Grok (xAI) API.
/// Uses OpenAI-compatible endpoint at https://api.x.ai/v1
pub struct GrokClient {
    http_client: Client,
    api_key: String,
    base_url: String,
    pub model_primary: String,
    pub model_fast: String,
    /// Track total tokens used for cost monitoring.
    total_tokens_used: AtomicU64,
}

/// Chat completion request body.
#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<Tool>>,
}

/// Chat message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Response format specification for structured output.
#[derive(Debug, Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    format_type: String,
}

/// Tool (function calling) definition.
#[derive(Debug, Serialize, Deserialize)]
pub struct Tool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDef,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FunctionDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Chat completion response from xAI API.
#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ResponseMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    content: Option<String>,
    tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

#[derive(Debug, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Deserialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

/// Result from a Grok API call.
#[derive(Debug)]
pub struct GrokResponse {
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tokens_used: u32,
    pub finish_reason: String,
}

impl GrokClient {
    pub fn new(config: &GrokConfig) -> Self {
        Self {
            http_client: Client::new(),
            api_key: config.api_key.clone(),
            base_url: config.base_url.clone(),
            model_primary: config.model_primary.clone(),
            model_fast: config.model_fast.clone(),
            total_tokens_used: AtomicU64::new(0),
        }
    }

    /// Send a chat completion request to Grok API.
    pub async fn chat(
        &self,
        messages: Vec<ChatMessage>,
        use_primary_model: bool,
        temperature: Option<f64>,
        max_tokens: Option<u32>,
    ) -> Result<GrokResponse> {
        let model = if use_primary_model {
            &self.model_primary
        } else {
            &self.model_fast
        };

        let request = ChatCompletionRequest {
            model: model.clone(),
            messages,
            temperature,
            max_tokens,
            response_format: None,
            tools: None,
        };

        self.send_request(request).await
    }

    /// Send a chat completion request expecting JSON output.
    pub async fn chat_json(
        &self,
        messages: Vec<ChatMessage>,
        use_primary_model: bool,
        temperature: Option<f64>,
        max_tokens: Option<u32>,
    ) -> Result<GrokResponse> {
        let model = if use_primary_model {
            &self.model_primary
        } else {
            &self.model_fast
        };

        let request = ChatCompletionRequest {
            model: model.clone(),
            messages,
            temperature,
            max_tokens,
            response_format: Some(ResponseFormat {
                format_type: "json_object".to_string(),
            }),
            tools: None,
        };

        self.send_request(request).await
    }

    /// Send a chat completion request with function calling.
    pub async fn chat_with_tools(
        &self,
        messages: Vec<ChatMessage>,
        tools: Vec<Tool>,
        use_primary_model: bool,
    ) -> Result<GrokResponse> {
        let model = if use_primary_model {
            &self.model_primary
        } else {
            &self.model_fast
        };

        let request = ChatCompletionRequest {
            model: model.clone(),
            messages,
            temperature: Some(0.3),
            max_tokens: Some(4096),
            response_format: None,
            tools: Some(tools),
        };

        self.send_request(request).await
    }

    /// Internal method to send the request and parse the response.
    async fn send_request(&self, request: ChatCompletionRequest) -> Result<GrokResponse> {
        let url = format!("{}/chat/completions", self.base_url);

        debug!("Sending request to Grok API: model={}", request.model);

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send request to Grok API")?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            error!("Grok API error ({}): {}", status, error_body);
            anyhow::bail!("Grok API returned error {}: {}", status, error_body);
        }

        let completion: ChatCompletionResponse = response
            .json()
            .await
            .context("Failed to parse Grok API response")?;

        let tokens_used = completion
            .usage
            .as_ref()
            .map(|u| u.total_tokens)
            .unwrap_or(0);

        // Track total tokens
        self.total_tokens_used
            .fetch_add(tokens_used as u64, Ordering::Relaxed);

        let choice = completion
            .choices
            .into_iter()
            .next()
            .context("No choices in Grok API response")?;

        Ok(GrokResponse {
            content: choice.message.content.unwrap_or_default(),
            tool_calls: choice.message.tool_calls,
            tokens_used,
            finish_reason: choice.finish_reason.unwrap_or_default(),
        })
    }

    /// Get total tokens used across all API calls.
    pub fn total_tokens_used(&self) -> u64 {
        self.total_tokens_used.load(Ordering::Relaxed)
    }

    /// Create a system message for trading analysis.
    pub fn trading_system_prompt() -> ChatMessage {
        ChatMessage {
            role: "system".to_string(),
            content: r#"You are an expert quantitative trading analyst and AI decision-making engine.

Your role is to analyze financial market data and provide actionable trading signals. You must:

1. ANALYZE technical indicators, chart patterns, and market sentiment objectively
2. IDENTIFY high-probability trading setups with clear entry/exit points
3. ASSESS risk/reward ratios and position sizing recommendations
4. PROVIDE clear reasoning for every recommendation
5. NEVER guarantee profits — always acknowledge uncertainty
6. CONSIDER multiple timeframes for confluence
7. FLAG potential risks and market conditions that could invalidate the setup

When providing trading signals, always include:
- Action: strong_buy, buy, hold, sell, strong_sell
- Confidence: 0.0 to 1.0
- Entry price
- Stop loss price
- Take profit price(s)
- Risk/reward ratio
- Detailed reasoning

Always respond in valid JSON format when requested."#.to_string(),
        }
    }
}

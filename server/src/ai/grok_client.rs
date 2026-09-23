use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::Duration;
use tracing::{debug, error};

use crate::config::GrokConfig;

/// Whole-request deadline. Grok 4.5+ always reason, and a live search adds
/// another round trip, so reqwest's 30s default drops valid completions.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(180);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// Client for the Grok (xAI) API.
/// Uses the OpenAI-compatible endpoint at https://api.x.ai/v1
pub struct GrokClient {
    http_client: Client,
    api_key: RwLock<String>,
    base_url: RwLock<String>,
    pub model_primary: RwLock<String>,
    pub model_fast: RwLock<String>,
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
    /// Visible answer budget. Does not include reasoning tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    max_completion_tokens: Option<u32>,
    /// `low`, `medium`, `high`, or `xhigh`. Omitted requests default to `high`.
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<Tool>>,
    /// Live web and X search. Omit to answer from model knowledge only.
    #[serde(skip_serializing_if = "Option::is_none")]
    search_parameters: Option<SearchParameters>,
}

/// xAI live-search block. With no `sources`, mode `on` searches the web and X.
#[derive(Debug, Serialize)]
struct SearchParameters {
    mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_search_results: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    return_citations: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    from_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sources: Option<Vec<SearchSource>>,
}

#[derive(Debug, Serialize)]
struct SearchSource {
    #[serde(rename = "type")]
    source_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    included_x_handles: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct ToolCompletionRequest {
    model: String,
    messages: Vec<ModelMessage>,
    temperature: f64,
    max_completion_tokens: u32,
    reasoning_effort: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<Tool>>,
}

fn response_text(payload: &serde_json::Value) -> String {
    let mut text = String::new();
    if let Some(items) = payload.get("output").and_then(|value| value.as_array()) {
        for item in items {
            if let Some(parts) = item.get("content").and_then(|value| value.as_array()) {
                for part in parts {
                    if let Some(chunk) = part.get("text").and_then(|value| value.as_str()) {
                        text.push_str(chunk);
                    }
                }
            }
        }
    }
    if text.is_empty() {
        if let Some(chunk) = payload.get("output_text").and_then(|value| value.as_str()) {
            text = chunk.to_string();
        }
    }
    text
}

fn live_search(max_results: u32) -> SearchParameters {
    SearchParameters {
        mode: "on".to_string(),
        max_search_results: Some(max_results),
        return_citations: Some(false),
        from_date: None,
        sources: None,
    }
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Chat completion response from xAI API.
#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    #[serde(default)]
    model: String,
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

fn default_tool_type() -> String {
    "function".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type", default = "default_tool_type")]
    pub call_type: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

/// A chat-completions message that can carry tool calls and tool results.
#[derive(Debug, Clone, Serialize)]
pub struct ModelMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ModelMessage {
    pub fn text(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn assistant_tools(content: Option<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.filter(|text| !text.is_empty()),
            tool_calls: Some(tool_calls),
            tool_call_id: None,
        }
    }

    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: "tool".to_string(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
        }
    }
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
    pub model: String,
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tokens_used: u32,
    pub finish_reason: String,
}

impl GrokClient {
    pub fn new(config: &GrokConfig) -> Self {
        let http_client = Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("failed to build Grok HTTP client");

        Self {
            http_client,
            api_key: RwLock::new(config.api_key.clone()),
            base_url: RwLock::new(config.base_url.clone()),
            model_primary: RwLock::new(config.model_primary.clone()),
            model_fast: RwLock::new(config.model_fast.clone()),
            total_tokens_used: AtomicU64::new(0),
        }
    }

    pub fn set_api_key(&self, key: String) {
        if let Ok(mut lock) = self.api_key.write() {
            *lock = key;
        }
    }

    pub fn get_api_key(&self) -> String {
        self.api_key.read().map(|k| k.clone()).unwrap_or_default()
    }

    pub fn set_models(&self, primary: String, fast: String) {
        if let Ok(mut p) = self.model_primary.write() {
            *p = primary;
        }
        if let Ok(mut f) = self.model_fast.write() {
            *f = fast;
        }
    }

    pub fn set_base_url(&self, url: String) {
        if let Ok(mut b) = self.base_url.write() {
            *b = url;
        }
    }

    pub fn get_model_primary(&self) -> String {
        self.model_primary.read().map(|m| m.clone()).unwrap_or_else(|_| "grok-4.6".to_string())
    }

    pub fn get_model_fast(&self) -> String {
        self.model_fast.read().map(|m| m.clone()).unwrap_or_else(|_| "grok-4.5".to_string())
    }

    pub fn get_base_url(&self) -> String {
        self.base_url.read().map(|b| b.clone()).unwrap_or_else(|_| "https://api.x.ai/v1".to_string())
    }

    /// Send a chat completion request to Grok API.
    pub async fn chat(
        &self,
        messages: Vec<ChatMessage>,
        use_primary_model: bool,
        temperature: Option<f64>,
        max_completion_tokens: Option<u32>,
    ) -> Result<GrokResponse> {
        let request = self.build_request(
            messages,
            use_primary_model,
            temperature,
            max_completion_tokens,
            "low",
            None,
            None,
            None,
        );
        self.send_request(request, None).await
    }

    /// JSON completion.
    ///
    /// `reasoning_effort` is `low`, `medium`, `high`, or `xhigh`.
    /// `live_search_results` turns on web and X search when set.
    pub async fn chat_json(
        &self,
        messages: Vec<ChatMessage>,
        use_primary_model: bool,
        temperature: Option<f64>,
        max_completion_tokens: Option<u32>,
        reasoning_effort: &str,
        live_search_results: Option<u32>,
    ) -> Result<GrokResponse> {
        let request = self.build_request(
            messages,
            use_primary_model,
            temperature,
            max_completion_tokens,
            reasoning_effort,
            live_search_results,
            Some(ResponseFormat {
                format_type: "json_object".to_string(),
            }),
            None,
        );
        self.send_request(request, None).await
    }

    /// JSON completion on the fast model, searching only the given X accounts.
    /// Uses the Responses API X search tool. The old live-search field returns 410.
    pub async fn chat_x_accounts(
        &self,
        messages: Vec<ChatMessage>,
        handles: &[String],
        from_date: &str,
        max_completion_tokens: u32,
    ) -> Result<GrokResponse> {
        let body = serde_json::json!({
            "model": self.get_model_fast(),
            "input": messages.iter().map(|message| serde_json::json!({
                "role": message.role,
                "content": message.content,
            })).collect::<Vec<_>>(),
            "reasoning": { "effort": "low" },
            "max_output_tokens": max_completion_tokens,
            "text": { "format": { "type": "json_object" } },
            "tools": [{
                "type": "x_search",
                "allowed_x_handles": handles,
                "from_date": from_date,
            }]
        });
        self.post_responses(&body).await
    }

    async fn post_responses(&self, body: &serde_json::Value) -> Result<GrokResponse> {
        let url = format!("{}/responses", self.get_base_url());
        debug!("Sending X search request to Grok API at {url}");

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.get_api_key()))
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await
            .context("Failed to send request to Grok API")?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            error!("Grok API error ({}): {}", status, error_body);
            anyhow::bail!("Grok API returned error {}: {}", status, error_body);
        }

        let payload: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse Grok responses payload")?;
        let content = response_text(&payload);
        let tokens_used = payload
            .get("usage")
            .and_then(|usage| usage.get("total_tokens"))
            .and_then(|value| value.as_u64())
            .unwrap_or(0) as u32;
        self.total_tokens_used
            .fetch_add(tokens_used as u64, Ordering::Relaxed);

        Ok(GrokResponse {
            model: payload
                .get("model")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .to_string(),
            content,
            tool_calls: None,
            tokens_used,
            finish_reason: payload
                .get("status")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .to_string(),
        })
    }

    /// Send a chat completion request with function calling.
    pub async fn chat_with_tools(
        &self,
        messages: Vec<ChatMessage>,
        tools: Vec<Tool>,
        use_primary_model: bool,
    ) -> Result<GrokResponse> {
        let request = self.build_request(
            messages,
            use_primary_model,
            Some(0.3),
            Some(4096),
            "medium",
            None,
            None,
            Some(tools),
        );
        self.send_request(request, None).await
    }

    /// Tool-calling completion. `tools` is omitted when empty so the model must answer in text.
    pub async fn complete_with_tools(
        &self,
        messages: Vec<ModelMessage>,
        tools: Vec<Tool>,
    ) -> Result<GrokResponse> {
        let request = ToolCompletionRequest {
            model: self.get_model_primary(),
            messages,
            temperature: 0.2,
            max_completion_tokens: 2048,
            reasoning_effort: "medium".to_string(),
            tools: if tools.is_empty() { None } else { Some(tools) },
        };
        self.send_request(request, None).await
    }

    fn build_request(
        &self,
        messages: Vec<ChatMessage>,
        use_primary_model: bool,
        temperature: Option<f64>,
        max_completion_tokens: Option<u32>,
        reasoning_effort: &str,
        live_search_results: Option<u32>,
        response_format: Option<ResponseFormat>,
        tools: Option<Vec<Tool>>,
    ) -> ChatCompletionRequest {
        let model = if use_primary_model {
            self.get_model_primary()
        } else {
            self.get_model_fast()
        };

        ChatCompletionRequest {
            model,
            messages,
            temperature,
            max_completion_tokens,
            reasoning_effort: Some(reasoning_effort.to_string()),
            response_format,
            tools,
            search_parameters: live_search_results.map(live_search),
        }
    }

    /// Internal method to send the request and parse the response.
    /// `api_key_override` is used by the connection test before a key is saved.
    async fn send_request(
        &self,
        request: impl Serialize,
        api_key_override: Option<&str>,
    ) -> Result<GrokResponse> {
        let base_url = self.get_base_url();
        let api_key = match api_key_override {
            Some(key) if !key.trim().is_empty() => key.trim().to_string(),
            _ => self.get_api_key(),
        };
        let url = format!("{}/chat/completions", base_url);

        debug!("Sending request to Grok API at {url}");

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
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
            model: completion.model,
            content: choice.message.content.unwrap_or_default(),
            tool_calls: choice.message.tool_calls,
            tokens_used,
            finish_reason: choice.finish_reason.unwrap_or_default(),
        })
    }

    /// Test Grok connection with either current key or an override key
    pub async fn test_connection(&self, temp_key: Option<&str>) -> Result<String> {
        let key = match temp_key {
            Some(k) if !k.trim().is_empty() => k.trim().to_string(),
            _ => self.get_api_key(),
        };

        if key.is_empty() {
            anyhow::bail!("API key is empty or not configured");
        }

        let request = self.build_request(
            vec![ChatMessage {
                role: "user".to_string(),
                content: "Reply with the single word pong.".to_string(),
            }],
            false,
            Some(0.0),
            Some(32),
            "low",
            None,
            None,
            None,
        );

        let response = self
            .send_request(request, Some(&key))
            .await
            .context("Failed to reach Grok API endpoint")?;

        if response.content.trim().is_empty() {
            anyhow::bail!(
                "Grok API returned an empty completion (finish_reason={})",
                response.finish_reason
            );
        }

        Ok(format!(
            "Connection successful! Grok API verified ({}).",
            response.model
        ))
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

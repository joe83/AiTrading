use anyhow::Result;
use std::sync::Arc;
use tracing::{debug, info};

use super::grok_client::{ChatMessage, GrokClient};

/// Sentiment analysis result.
#[derive(Debug, Clone)]
pub struct SentimentResult {
    /// Overall sentiment score: -1.0 (very bearish) to 1.0 (very bullish).
    pub score: f64,
    /// Descriptive label.
    pub label: SentimentLabel,
    /// Human-readable summary from Grok.
    pub summary: String,
    /// Key themes/narratives detected.
    pub themes: Vec<String>,
    /// News-based sentiment (-1.0 to 1.0).
    pub news_score: Option<f64>,
    /// Social media sentiment (-1.0 to 1.0).
    pub social_score: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SentimentLabel {
    VeryBullish,
    Bullish,
    Neutral,
    Bearish,
    VeryBearish,
}

impl std::fmt::Display for SentimentLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SentimentLabel::VeryBullish => write!(f, "Very Bullish"),
            SentimentLabel::Bullish => write!(f, "Bullish"),
            SentimentLabel::Neutral => write!(f, "Neutral"),
            SentimentLabel::Bearish => write!(f, "Bearish"),
            SentimentLabel::VeryBearish => write!(f, "Very Bearish"),
        }
    }
}

/// Sentiment analyzer using Grok AI for market sentiment interpretation.
pub struct SentimentAnalyzer {
    grok: Arc<GrokClient>,
}

impl SentimentAnalyzer {
    pub fn new(grok: Arc<GrokClient>) -> Self {
        Self { grok }
    }

    /// Analyze market sentiment for a given symbol.
    pub async fn analyze(
        &self,
        symbol: &str,
        additional_context: Option<&str>,
    ) -> Result<SentimentResult> {
        info!("Running sentiment analysis for {symbol}");

        let mut context_text = String::new();
        if let Some(ctx) = additional_context {
            context_text = format!("\n\nAdditional context:\n{ctx}");
        }

        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: r#"You are a market sentiment analyst. Analyze the current market sentiment for the given asset.

Provide your analysis in JSON format:
{
    "score": <float from -1.0 (very bearish) to 1.0 (very bullish)>,
    "label": "<very_bearish|bearish|neutral|bullish|very_bullish>",
    "summary": "<2-3 sentence summary of current sentiment>",
    "themes": ["<key theme 1>", "<key theme 2>", ...],
    "news_sentiment": <float from -1.0 to 1.0 or null>,
    "social_sentiment": <float from -1.0 to 1.0 or null>,
    "risk_factors": ["<risk 1>", "<risk 2>", ...]
}

Base your analysis on your knowledge of:
- Recent market trends and price action
- Macroeconomic conditions
- Sector/industry sentiment
- Known upcoming events (earnings, Fed meetings, etc.)
- General market fear/greed indicators"#
                    .to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: format!(
                    "Analyze the current market sentiment for: {symbol}{context_text}\n\nProvide your analysis in JSON format."
                ),
            },
        ];

        let response = self
            .grok
            .chat_json(messages, false, Some(0.3), Some(1024))
            .await?;

        // Parse the JSON response
        let parsed: serde_json::Value = serde_json::from_str(&response.content)
            .unwrap_or_else(|_| {
                debug!("Failed to parse Grok sentiment response as JSON, using defaults");
                serde_json::json!({
                    "score": 0.0,
                    "label": "neutral",
                    "summary": "Unable to determine sentiment",
                    "themes": [],
                    "news_sentiment": null,
                    "social_sentiment": null,
                })
            });

        let score = parsed["score"].as_f64().unwrap_or(0.0).clamp(-1.0, 1.0);

        let label = match parsed["label"].as_str().unwrap_or("neutral") {
            "very_bullish" => SentimentLabel::VeryBullish,
            "bullish" => SentimentLabel::Bullish,
            "bearish" => SentimentLabel::Bearish,
            "very_bearish" => SentimentLabel::VeryBearish,
            _ => SentimentLabel::Neutral,
        };

        let themes: Vec<String> = parsed["themes"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|t| t.as_str().map(String::from))
            .collect();

        Ok(SentimentResult {
            score,
            label,
            summary: parsed["summary"]
                .as_str()
                .unwrap_or("No summary available")
                .to_string(),
            themes,
            news_score: parsed["news_sentiment"].as_f64(),
            social_score: parsed["social_sentiment"].as_f64(),
        })
    }
}

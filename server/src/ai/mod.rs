pub mod agent;
pub mod grok_client;
pub mod watch_loop;
pub mod technical_analysis;
pub mod tools;
pub mod sentiment_analysis;
pub mod pattern_recognition;
pub mod decision_maker;
pub mod meme_radar;

use anyhow::Result;
use std::sync::Arc;
use tracing::{info, warn};

use crate::config::AppConfig;
use crate::db::Database;
use crate::models::*;

use self::decision_maker::DecisionMaker;
use self::grok_client::GrokClient;
use self::meme_radar::MemeRadarEngine;
use self::pattern_recognition::PatternRecognizer;
use self::sentiment_analysis::SentimentAnalyzer;
use self::technical_analysis::TechnicalAnalyzer;

/// AI Analysis Engine — orchestrates all analysis modules and produces trading signals.
pub struct AiEngine {
    pub grok: Arc<GrokClient>,
    pub technical: TechnicalAnalyzer,
    pub sentiment: SentimentAnalyzer,
    pub pattern: PatternRecognizer,
    pub decision_maker: DecisionMaker,
    pub meme_radar: Arc<MemeRadarEngine>,
    db: Arc<Database>,
}

impl AiEngine {
    pub fn new(config: &AppConfig, db: Arc<Database>) -> Self {
        let grok = Arc::new(GrokClient::new(&config.grok));
        let meme_radar = Arc::new(MemeRadarEngine::new(grok.clone()));

        Self {
            grok: grok.clone(),
            technical: TechnicalAnalyzer::new(),
            sentiment: SentimentAnalyzer::new(grok.clone()),
            pattern: PatternRecognizer::new(),
            decision_maker: DecisionMaker::new(grok.clone(), &config.trading, db.clone()),
            meme_radar,
            db,
        }
    }

    /// Run full analysis on a symbol across multiple timeframes.
    /// Returns a comprehensive AnalysisResult with a trading signal.
    pub async fn analyze(
        &self,
        symbol: &str,
        exchange: ExchangeId,
        candles: &CandleSeries,
        additional_context: Option<&str>,
    ) -> Result<AnalysisResult> {
        info!("Starting AI analysis for {symbol} on {exchange}");

        // Step 1: Technical Analysis
        let tech_result = self.technical.analyze(candles)?;
        info!("Technical analysis complete for {symbol}: {:?}", tech_result.summary);

        // Step 2: Pattern Recognition
        let patterns = self.pattern.detect(candles)?;
        info!(
            "Pattern recognition complete for {symbol}: {} patterns found",
            patterns.len()
        );

        // Step 3: Sentiment Analysis (async — calls Grok API)
        let sentiment = self.sentiment.analyze(symbol, additional_context).await?;
        info!("Sentiment analysis complete for {symbol}: score={}", sentiment.score);

        // Step 4: AI Decision Making (async — calls Grok API)
        let (signal, decision_tokens, model) = self
            .decision_maker
            .decide(symbol, exchange, &tech_result, &patterns, &sentiment, candles)
            .await?;
        info!(
            "AI decision for {symbol}: {:?} (confidence: {:.1}%)",
            signal.action,
            signal.confidence * 100.0
        );

        let result = AnalysisResult {
            id: uuid::Uuid::new_v4(),
            symbol: symbol.to_string(),
            exchange,
            timestamp: chrono::Utc::now(),
            technical_summary: tech_result.summary.clone(),
            sentiment_summary: sentiment.summary.clone(),
            pattern_summary: patterns
                .iter()
                .map(|p| format!("{}: {:.0}%", p.name, p.confidence * 100.0))
                .collect::<Vec<_>>()
                .join(", "),
            overall_analysis: signal.reasoning.clone(),
            signal,
            grok_model_used: model,
            tokens_used: sentiment.tokens_used.saturating_add(decision_tokens),
        };

        if let Err(e) = self.db.insert_signal(&result.signal).await {
            warn!("Failed to persist signal {}: {e}", result.signal.id);
        }
        if let Err(e) = self.db.insert_analysis(&result).await {
            warn!("Failed to persist analysis {}: {e}", result.id);
        }

        Ok(result)
    }
}

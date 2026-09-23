use anyhow::Result;
use rust_decimal::Decimal;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

use super::grok_client::{ChatMessage, GrokClient};
use super::pattern_recognition::{DetectedPattern, PatternDirection};
use super::sentiment_analysis::SentimentResult;
use super::technical_analysis::{TechnicalResult, TrendDirection};
use crate::config::TradingConfig;
use crate::db::{format_playbook, Database};
use crate::models::*;

/// The AI Decision Maker — the "brain" that synthesizes all analysis into actionable trading signals.
pub struct DecisionMaker {
    grok: Arc<GrokClient>,
    db: Arc<Database>,
    max_position_size_pct: f64,
    default_stop_loss_pct: f64,
    default_take_profit_pct: f64,
}

impl DecisionMaker {
    pub fn new(grok: Arc<GrokClient>, config: &TradingConfig, db: Arc<Database>) -> Self {
        Self {
            grok,
            db,
            max_position_size_pct: config.max_position_size_pct,
            default_stop_loss_pct: config.default_stop_loss_pct,
            default_take_profit_pct: config.default_take_profit_pct,
        }
    }

    /// Make a trading decision by combining technical, sentiment, and pattern analysis.
    /// Sends aggregated data to Grok for final synthesis and decision.
    pub async fn decide(
        &self,
        symbol: &str,
        exchange: ExchangeId,
        technical: &TechnicalResult,
        patterns: &[DetectedPattern],
        sentiment: &SentimentResult,
        candles: &CandleSeries,
    ) -> Result<(TradingSignal, u32, String)> {
        let closes = candles.closes_f64();
        let current_price = *closes.last().unwrap_or(&0.0);

        let playbook = self.playbook_text(symbol).await;
        let analysis_summary = self.build_analysis_summary(
            symbol,
            current_price,
            technical,
            patterns,
            sentiment,
            &playbook,
        );

        // Call Grok AI for final decision
        let messages = vec![
            GrokClient::trading_system_prompt(),
            ChatMessage {
                role: "user".to_string(),
                content: format!(
                    r#"Based on the following comprehensive analysis for {symbol}, provide your trading decision.

{analysis_summary}

Respond in JSON format:
{{
    "action": "<strong_buy|buy|hold|sell|strong_sell>",
    "confidence": <0.0 to 1.0>,
    "entry_price": <recommended entry price or null>,
    "stop_loss": <recommended stop loss price>,
    "take_profit": <recommended take profit price>,
    "position_size_pct": <recommended position size as % of portfolio, max {max_pct}%>,
    "risk_reward_ratio": <calculated risk/reward ratio>,
    "reasoning": "<detailed 2-3 sentence reasoning for the decision>",
    "key_factors": ["<factor1>", "<factor2>", ...],
    "invalidation": "<what would invalidate this trade setup>"
}}"#,
                    max_pct = self.max_position_size_pct
                ),
            },
        ];

        let response = self
            .grok
            .chat_json(messages, true, Some(0.2), Some(2048), "medium", None)
            .await?;
        let tokens_used = response.tokens_used;
        let model = response.model.clone();

        // Parse Grok's decision
        let decision: serde_json::Value = serde_json::from_str(&response.content)
            .unwrap_or_else(|e| {
                warn!("Failed to parse Grok decision JSON: {e}. Using fallback.");
                self.generate_fallback_decision(technical, sentiment, patterns, current_price)
            });

        let action = match decision["action"].as_str().unwrap_or("hold") {
            "strong_buy" => SignalAction::StrongBuy,
            "buy" => SignalAction::Buy,
            "sell" => SignalAction::Sell,
            "strong_sell" => SignalAction::StrongSell,
            _ => SignalAction::Hold,
        };

        let confidence = decision["confidence"].as_f64().unwrap_or(0.5).clamp(0.0, 1.0);

        let mut signal = TradingSignal::new(
            exchange,
            symbol,
            &candles.timeframe,
            action,
            SignalSource::AiDecision,
            confidence,
            decision["reasoning"].as_str().unwrap_or("AI decision based on multi-factor analysis"),
        );

        // Set price levels
        signal.entry_price = decision["entry_price"]
            .as_f64()
            .and_then(|p| Decimal::try_from(p).ok());

        signal.stop_loss = decision["stop_loss"]
            .as_f64()
            .and_then(|p| Decimal::try_from(p).ok())
            .or_else(|| {
                // Fallback: use ATR-based or percentage-based stop loss
                let sl = current_price * (1.0 - self.default_stop_loss_pct / 100.0);
                Decimal::try_from(sl).ok()
            });

        signal.take_profit = decision["take_profit"]
            .as_f64()
            .and_then(|p| Decimal::try_from(p).ok())
            .or_else(|| {
                let tp = current_price * (1.0 + self.default_take_profit_pct / 100.0);
                Decimal::try_from(tp).ok()
            });

        signal.position_size_pct = decision["position_size_pct"]
            .as_f64()
            .map(|p| p.min(self.max_position_size_pct));

        signal.risk_reward_ratio = decision["risk_reward_ratio"].as_f64();

        // Populate indicator snapshot
        signal.indicators = SignalIndicators {
            rsi: technical.rsi,
            macd: technical.macd.as_ref().map(|m| MacdValues {
                macd: m.macd_line,
                signal: m.signal_line,
                histogram: m.histogram,
            }),
            ema_short: technical.ema_short,
            ema_long: technical.ema_long,
            bollinger: technical.bollinger.as_ref().map(|b| BollingerValues {
                upper: b.upper,
                middle: b.middle,
                lower: b.lower,
                bandwidth: b.bandwidth,
            }),
            atr: technical.atr,
            volume_ratio: technical.volume_ratio,
            stochastic_k: technical.stochastic_k,
            stochastic_d: technical.stochastic_d,
            sentiment_score: Some(sentiment.score),
            pattern_name: patterns.first().map(|p| p.name.clone()),
            pattern_confidence: patterns.first().map(|p| p.confidence),
        };

        info!(
            "Decision for {symbol}: {:?} (confidence: {:.1}%)",
            signal.action,
            signal.confidence * 100.0
        );

        Ok((signal, tokens_used, model))
    }

    /// Build comprehensive analysis summary string for Grok.
    fn build_analysis_summary(
        &self,
        symbol: &str,
        current_price: f64,
        technical: &TechnicalResult,
        patterns: &[DetectedPattern],
        sentiment: &SentimentResult,
        playbook: &str,
    ) -> String {
        let mut summary = format!("=== {symbol} Analysis ===\n\n");

        summary.push_str(&format!("Current Price: {current_price:.6}\n\n"));

        // Technical Analysis
        summary.push_str("--- TECHNICAL ANALYSIS ---\n");
        summary.push_str(&format!("Overall Trend: {} (strength: {:.0}%)\n", technical.trend, technical.strength * 100.0));

        if let Some(rsi) = technical.rsi {
            let condition = if rsi > 70.0 { "OVERBOUGHT" } else if rsi < 30.0 { "OVERSOLD" } else { "Normal" };
            summary.push_str(&format!("RSI(14): {rsi:.1} [{condition}]\n"));
        }
        if let Some(macd) = &technical.macd {
            let signal_type = if macd.histogram > 0.0 { "BULLISH" } else { "BEARISH" };
            summary.push_str(&format!("MACD: {:.6} | Signal: {:.6} | Histogram: {:.6} [{signal_type}]\n", macd.macd_line, macd.signal_line, macd.histogram));
        }
        if let Some(bb) = &technical.bollinger {
            summary.push_str(&format!("Bollinger: Upper={:.6} Mid={:.6} Lower={:.6} (%B={:.2})\n", bb.upper, bb.middle, bb.lower, bb.percent_b));
        }
        if let (Some(ema_s), Some(ema_l)) = (technical.ema_short, technical.ema_long) {
            let cross = if ema_s > ema_l { "BULLISH" } else { "BEARISH" };
            summary.push_str(&format!("EMA(9): {ema_s:.6} | EMA(21): {ema_l:.6} [{cross} crossover]\n"));
        }
        if let Some(atr) = technical.atr {
            summary.push_str(&format!("ATR(14): {atr:.6} (volatility measure)\n"));
        }
        if let (Some(k), Some(d)) = (technical.stochastic_k, technical.stochastic_d) {
            summary.push_str(&format!("Stochastic: %K={k:.1} %D={d:.1}\n"));
        }
        if let Some(vr) = technical.volume_ratio {
            summary.push_str(&format!("Volume Ratio: {vr:.2}x (vs 20-SMA)\n"));
        }

        // Pattern Analysis
        summary.push_str("\n--- PATTERN ANALYSIS ---\n");
        if patterns.is_empty() {
            summary.push_str("No significant patterns detected.\n");
        } else {
            for p in patterns {
                summary.push_str(&format!(
                    "• {} [{:?}] - Confidence: {:.0}% — {}\n",
                    p.name,
                    p.direction,
                    p.confidence * 100.0,
                    p.description
                ));
            }
        }

        // Sentiment Analysis
        summary.push_str("\n--- SENTIMENT ANALYSIS ---\n");
        summary.push_str(&format!("Score: {:.2} ({})\n", sentiment.score, sentiment.label));
        summary.push_str(&format!("Summary: {}\n", sentiment.summary));
        if !sentiment.themes.is_empty() {
            summary.push_str(&format!("Key Themes: {}\n", sentiment.themes.join(", ")));
        }

        // Risk Parameters
        summary.push_str("\n--- RISK PARAMETERS ---\n");
        summary.push_str(&format!("Max Position Size: {:.1}%\n", self.max_position_size_pct));
        summary.push_str(&format!("Default Stop Loss: {:.1}%\n", self.default_stop_loss_pct));
        summary.push_str(&format!("Default Take Profit: {:.1}%\n", self.default_take_profit_pct));
        summary.push_str(playbook);

        summary
    }

    /// Load approved rules. A slow or failed lookup leaves the decision on the market data alone.
    async fn playbook_text(&self, symbol: &str) -> String {
        match tokio::time::timeout(Duration::from_secs(2), self.db.active_playbook(symbol)).await {
            Ok(Ok(rules)) => format_playbook(&rules),
            Ok(Err(e)) => {
                warn!("Playbook lookup failed for {symbol}: {e}");
                String::new()
            }
            Err(_) => {
                warn!("Playbook lookup timed out for {symbol}");
                String::new()
            }
        }
    }

    /// Generate a fallback decision if Grok API fails or returns invalid JSON.
    fn generate_fallback_decision(
        &self,
        technical: &TechnicalResult,
        sentiment: &SentimentResult,
        patterns: &[DetectedPattern],
        current_price: f64,
    ) -> serde_json::Value {
        // Simple rule-based fallback
        let mut score = 0.0f64;

        // Technical trend
        match technical.trend {
            TrendDirection::StrongBullish => score += 2.0,
            TrendDirection::Bullish => score += 1.0,
            TrendDirection::Bearish => score -= 1.0,
            TrendDirection::StrongBearish => score -= 2.0,
            TrendDirection::Neutral => {}
        }

        // Sentiment
        score += sentiment.score;

        // Pattern confluence
        for p in patterns {
            match p.direction {
                PatternDirection::Bullish => score += p.confidence * 0.5,
                PatternDirection::Bearish => score -= p.confidence * 0.5,
                PatternDirection::Neutral => {}
            }
        }

        let (action, confidence) = if score > 2.0 {
            ("strong_buy", 0.75)
        } else if score > 1.0 {
            ("buy", 0.6)
        } else if score < -2.0 {
            ("strong_sell", 0.75)
        } else if score < -1.0 {
            ("sell", 0.6)
        } else {
            ("hold", 0.5)
        };

        let sl = current_price * (1.0 - self.default_stop_loss_pct / 100.0);
        let tp = current_price * (1.0 + self.default_take_profit_pct / 100.0);

        serde_json::json!({
            "action": action,
            "confidence": confidence,
            "entry_price": current_price,
            "stop_loss": sl,
            "take_profit": tp,
            "position_size_pct": self.max_position_size_pct * 0.5,
            "risk_reward_ratio": (tp - current_price) / (current_price - sl),
            "reasoning": format!("Fallback rule-based decision: aggregate score = {score:.1}. Technical trend: {}, Sentiment: {}.", technical.trend, sentiment.label),
            "key_factors": ["technical_trend", "sentiment_score"],
            "invalidation": "If price breaks below stop loss level"
        })
    }
}

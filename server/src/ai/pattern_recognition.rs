use anyhow::Result;
use rust_decimal::Decimal;
use tracing::debug;

use crate::models::CandleSeries;

/// Detected chart/candlestick pattern.
#[derive(Debug, Clone)]
pub struct DetectedPattern {
    pub name: String,
    pub pattern_type: PatternType,
    pub direction: PatternDirection,
    pub confidence: f64,
    pub description: String,
    /// Index in the candle series where pattern starts.
    pub start_index: usize,
    /// Index in the candle series where pattern ends.
    pub end_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PatternType {
    Candlestick,
    Chart,
    SupportResistance,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PatternDirection {
    Bullish,
    Bearish,
    Neutral,
}

/// Pattern recognition engine for candlestick and chart patterns.
pub struct PatternRecognizer;

impl PatternRecognizer {
    pub fn new() -> Self {
        Self
    }

    /// Detect all recognizable patterns in a candle series.
    pub fn detect(&self, candles: &CandleSeries) -> Result<Vec<DetectedPattern>> {
        let mut patterns = Vec::new();

        if candles.candles.len() < 5 {
            return Ok(patterns);
        }

        // Candlestick patterns
        self.detect_doji(candles, &mut patterns);
        self.detect_hammer(candles, &mut patterns);
        self.detect_engulfing(candles, &mut patterns);
        self.detect_morning_evening_star(candles, &mut patterns);
        self.detect_three_soldiers_crows(candles, &mut patterns);

        // Support/Resistance
        self.detect_support_resistance(candles, &mut patterns);

        debug!("Detected {} patterns", patterns.len());
        Ok(patterns)
    }

    /// Detect Doji patterns (indecision candle).
    fn detect_doji(&self, candles: &CandleSeries, patterns: &mut Vec<DetectedPattern>) {
        let threshold = Decimal::new(1, 1); // 0.1 = 10% body-to-range ratio
        let len = candles.candles.len();

        for i in (len.saturating_sub(3))..len {
            let candle = &candles.candles[i];
            if candle.is_doji(threshold) && !candle.range().is_zero() {
                patterns.push(DetectedPattern {
                    name: "Doji".to_string(),
                    pattern_type: PatternType::Candlestick,
                    direction: PatternDirection::Neutral,
                    confidence: 0.6,
                    description: "Doji detected — indicates indecision. Potential reversal signal when occurring after a trend.".to_string(),
                    start_index: i,
                    end_index: i,
                });
            }
        }
    }

    /// Detect Hammer / Inverted Hammer patterns.
    fn detect_hammer(&self, candles: &CandleSeries, patterns: &mut Vec<DetectedPattern>) {
        let len = candles.candles.len();

        for i in (len.saturating_sub(3))..len {
            let candle = &candles.candles[i];
            let range = candle.range();
            if range.is_zero() {
                continue;
            }

            let body = candle.body_size();
            let lower_shadow = candle.lower_shadow();
            let upper_shadow = candle.upper_shadow();

            // Hammer: small body at top, long lower shadow (2x body)
            if lower_shadow > body * Decimal::new(2, 0)
                && upper_shadow < body
                && body > Decimal::ZERO
            {
                patterns.push(DetectedPattern {
                    name: "Hammer".to_string(),
                    pattern_type: PatternType::Candlestick,
                    direction: PatternDirection::Bullish,
                    confidence: 0.65,
                    description: "Hammer pattern — bullish reversal signal at support levels."
                        .to_string(),
                    start_index: i,
                    end_index: i,
                });
            }

            // Inverted Hammer: small body at bottom, long upper shadow
            if upper_shadow > body * Decimal::new(2, 0)
                && lower_shadow < body
                && body > Decimal::ZERO
            {
                patterns.push(DetectedPattern {
                    name: "Inverted Hammer".to_string(),
                    pattern_type: PatternType::Candlestick,
                    direction: PatternDirection::Bullish,
                    confidence: 0.55,
                    description: "Inverted hammer — potential bullish reversal, needs confirmation."
                        .to_string(),
                    start_index: i,
                    end_index: i,
                });
            }

            // Shooting Star: like inverted hammer but at top of uptrend
            if upper_shadow > body * Decimal::new(2, 0)
                && lower_shadow < body
                && candle.is_bearish()
                && i >= 2
                && candles.candles[i - 1].is_bullish()
                && candles.candles[i - 2].is_bullish()
            {
                patterns.push(DetectedPattern {
                    name: "Shooting Star".to_string(),
                    pattern_type: PatternType::Candlestick,
                    direction: PatternDirection::Bearish,
                    confidence: 0.65,
                    description: "Shooting star after uptrend — bearish reversal signal."
                        .to_string(),
                    start_index: i,
                    end_index: i,
                });
            }
        }
    }

    /// Detect Engulfing patterns (bullish and bearish).
    fn detect_engulfing(&self, candles: &CandleSeries, patterns: &mut Vec<DetectedPattern>) {
        let len = candles.candles.len();

        for i in (len.saturating_sub(5).max(1))..len {
            let prev = &candles.candles[i - 1];
            let curr = &candles.candles[i];

            // Bullish Engulfing: bearish candle followed by larger bullish candle
            if prev.is_bearish()
                && curr.is_bullish()
                && curr.open < prev.close
                && curr.close > prev.open
                && curr.body_size() > prev.body_size()
            {
                patterns.push(DetectedPattern {
                    name: "Bullish Engulfing".to_string(),
                    pattern_type: PatternType::Candlestick,
                    direction: PatternDirection::Bullish,
                    confidence: 0.7,
                    description: "Bullish engulfing pattern — strong reversal signal at support."
                        .to_string(),
                    start_index: i - 1,
                    end_index: i,
                });
            }

            // Bearish Engulfing: bullish candle followed by larger bearish candle
            if prev.is_bullish()
                && curr.is_bearish()
                && curr.open > prev.close
                && curr.close < prev.open
                && curr.body_size() > prev.body_size()
            {
                patterns.push(DetectedPattern {
                    name: "Bearish Engulfing".to_string(),
                    pattern_type: PatternType::Candlestick,
                    direction: PatternDirection::Bearish,
                    confidence: 0.7,
                    description: "Bearish engulfing pattern — strong reversal signal at resistance."
                        .to_string(),
                    start_index: i - 1,
                    end_index: i,
                });
            }
        }
    }

    /// Detect Morning Star / Evening Star patterns.
    fn detect_morning_evening_star(&self, candles: &CandleSeries, patterns: &mut Vec<DetectedPattern>) {
        let len = candles.candles.len();

        for i in (len.saturating_sub(5).max(2))..len {
            let first = &candles.candles[i - 2];
            let middle = &candles.candles[i - 1];
            let third = &candles.candles[i];

            let threshold = Decimal::new(3, 1); // 0.3

            // Morning Star: bearish → small body (doji-like) → bullish
            if first.is_bearish()
                && first.body_size() > middle.body_size() * Decimal::new(2, 0)
                && middle.is_doji(threshold)
                && third.is_bullish()
                && third.close > first.body_size() / Decimal::new(2, 0) + first.close
            {
                patterns.push(DetectedPattern {
                    name: "Morning Star".to_string(),
                    pattern_type: PatternType::Candlestick,
                    direction: PatternDirection::Bullish,
                    confidence: 0.75,
                    description: "Morning star pattern — strong bullish reversal.".to_string(),
                    start_index: i - 2,
                    end_index: i,
                });
            }

            // Evening Star: bullish → small body → bearish
            if first.is_bullish()
                && first.body_size() > middle.body_size() * Decimal::new(2, 0)
                && middle.is_doji(threshold)
                && third.is_bearish()
                && third.close < first.open - first.body_size() / Decimal::new(2, 0)
            {
                patterns.push(DetectedPattern {
                    name: "Evening Star".to_string(),
                    pattern_type: PatternType::Candlestick,
                    direction: PatternDirection::Bearish,
                    confidence: 0.75,
                    description: "Evening star pattern — strong bearish reversal.".to_string(),
                    start_index: i - 2,
                    end_index: i,
                });
            }
        }
    }

    /// Detect Three White Soldiers / Three Black Crows.
    fn detect_three_soldiers_crows(&self, candles: &CandleSeries, patterns: &mut Vec<DetectedPattern>) {
        let len = candles.candles.len();

        if len < 3 {
            return;
        }

        for i in (len.saturating_sub(5).max(2))..len {
            let c1 = &candles.candles[i - 2];
            let c2 = &candles.candles[i - 1];
            let c3 = &candles.candles[i];

            // Three White Soldiers
            if c1.is_bullish()
                && c2.is_bullish()
                && c3.is_bullish()
                && c2.close > c1.close
                && c3.close > c2.close
                && c2.open > c1.open
                && c3.open > c2.open
            {
                patterns.push(DetectedPattern {
                    name: "Three White Soldiers".to_string(),
                    pattern_type: PatternType::Candlestick,
                    direction: PatternDirection::Bullish,
                    confidence: 0.8,
                    description: "Three consecutive bullish candles with higher closes — strong uptrend continuation.".to_string(),
                    start_index: i - 2,
                    end_index: i,
                });
            }

            // Three Black Crows
            if c1.is_bearish()
                && c2.is_bearish()
                && c3.is_bearish()
                && c2.close < c1.close
                && c3.close < c2.close
                && c2.open < c1.open
                && c3.open < c2.open
            {
                patterns.push(DetectedPattern {
                    name: "Three Black Crows".to_string(),
                    pattern_type: PatternType::Candlestick,
                    direction: PatternDirection::Bearish,
                    confidence: 0.8,
                    description: "Three consecutive bearish candles with lower closes — strong downtrend continuation.".to_string(),
                    start_index: i - 2,
                    end_index: i,
                });
            }
        }
    }

    /// Detect support and resistance levels from price action.
    fn detect_support_resistance(&self, candles: &CandleSeries, patterns: &mut Vec<DetectedPattern>) {
        let closes = candles.closes_f64();
        let highs = candles.highs_f64();
        let lows = candles.lows_f64();

        if closes.len() < 20 {
            return;
        }

        // Find pivot highs and lows (local extremes)
        let lookback = 5;
        let mut support_levels = Vec::new();
        let mut resistance_levels = Vec::new();

        for i in lookback..(closes.len() - lookback) {
            // Pivot high
            let is_pivot_high = (1..=lookback).all(|j| {
                highs[i] >= highs[i - j] && highs[i] >= highs[i + j]
            });
            if is_pivot_high {
                resistance_levels.push(highs[i]);
            }

            // Pivot low
            let is_pivot_low = (1..=lookback).all(|j| {
                lows[i] <= lows[i - j] && lows[i] <= lows[i + j]
            });
            if is_pivot_low {
                support_levels.push(lows[i]);
            }
        }

        // Cluster nearby levels
        let current_price = *closes.last().unwrap_or(&0.0);

        // Find nearest support
        if let Some(nearest_support) = support_levels
            .iter()
            .filter(|&&s| s < current_price)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        {
            let distance_pct = ((current_price - nearest_support) / current_price) * 100.0;
            if distance_pct < 5.0 {
                patterns.push(DetectedPattern {
                    name: format!("Support at {nearest_support:.2}"),
                    pattern_type: PatternType::SupportResistance,
                    direction: PatternDirection::Bullish,
                    confidence: 0.6,
                    description: format!(
                        "Price is {distance_pct:.1}% above support at {nearest_support:.2}"
                    ),
                    start_index: candles.candles.len() - 1,
                    end_index: candles.candles.len() - 1,
                });
            }
        }

        // Find nearest resistance
        if let Some(nearest_resistance) = resistance_levels
            .iter()
            .filter(|&&r| r > current_price)
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        {
            let distance_pct = ((nearest_resistance - current_price) / current_price) * 100.0;
            if distance_pct < 5.0 {
                patterns.push(DetectedPattern {
                    name: format!("Resistance at {nearest_resistance:.2}"),
                    pattern_type: PatternType::SupportResistance,
                    direction: PatternDirection::Bearish,
                    confidence: 0.6,
                    description: format!(
                        "Price is {distance_pct:.1}% below resistance at {nearest_resistance:.2}"
                    ),
                    start_index: candles.candles.len() - 1,
                    end_index: candles.candles.len() - 1,
                });
            }
        }
    }
}

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use sqlx::Row;
use uuid::Uuid;

use crate::models::{AnalysisResult, ExchangeId, OrderSide, SignalAction, SignalSource, TradingSignal};

use super::Database;

/// One approved rule the decision prompt is allowed to see.
#[derive(Debug, Clone)]
pub struct PlaybookRule {
    pub scope: String,
    pub rule: String,
    pub sample_size: i32,
}

/// A playbook row returned to the dashboard.
#[derive(Debug, Clone)]
pub struct PlaybookEntry {
    pub id: Uuid,
    pub lesson_id: Option<Uuid>,
    pub scope: String,
    pub rule: String,
    pub sample_size: i32,
    pub created_at: DateTime<Utc>,
}

/// Fields written when a trade opens. The row is updated again when it closes.
pub struct TradeInsert {
    pub id: Uuid,
    pub exchange: ExchangeId,
    pub symbol: String,
    pub side: OrderSide,
    pub quantity: Decimal,
    pub entry_price: Decimal,
    pub fees: Decimal,
    pub signal_id: Uuid,
    pub signal_confidence: f64,
    pub opened_at: DateTime<Utc>,
}

/// Render approved rules for the decision prompt. Empty when there is nothing to add.
pub fn format_playbook(rules: &[PlaybookRule]) -> String {
    if rules.is_empty() {
        return String::new();
    }

    let mut out = String::from(
        "\n--- ACTIVE PLAYBOOK ---\nApproved rules from past trades. Respect a rule when it applies to this setup.\n",
    );
    for rule in rules.iter().take(12) {
        if rule.sample_size > 0 {
            out.push_str(&format!(
                "- [{}] {} (sample {})\n",
                rule.scope, rule.rule, rule.sample_size
            ));
        } else {
            out.push_str(&format!("- [{}] {}\n", rule.scope, rule.rule));
        }
    }
    out
}

/// Store `"global"` in lowercase and symbols in uppercase so lookups match.
pub fn normalize_scope(scope: &str) -> String {
    let trimmed = scope.trim();
    if trimmed.eq_ignore_ascii_case("global") {
        "global".to_string()
    } else {
        trimmed.to_uppercase()
    }
}

impl Database {
    pub async fn insert_signal(&self, signal: &TradingSignal) -> Result<()> {
        let indicators = serde_json::to_value(&signal.indicators).unwrap_or_else(|_| serde_json::json!({}));
        sqlx::query(
            r#"
            INSERT INTO signals (
                id, exchange, symbol, timeframe, action, source, confidence,
                entry_price, stop_loss, take_profit, position_size_pct, risk_reward_ratio,
                reasoning, indicators, executed, created_at, expires_at
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7,
                $8, $9, $10, $11, $12,
                $13, $14, $15, $16, $17
            )
            ON CONFLICT (id) DO NOTHING
            "#,
        )
        .bind(signal.id)
        .bind(signal.exchange.as_db_str())
        .bind(&signal.symbol)
        .bind(truncate_chars(&signal.timeframe, 8))
        .bind(action_db(signal.action))
        .bind(source_db(signal.source))
        .bind(signal.confidence)
        .bind(signal.entry_price)
        .bind(signal.stop_loss)
        .bind(signal.take_profit)
        .bind(signal.position_size_pct)
        .bind(signal.risk_reward_ratio)
        .bind(&signal.reasoning)
        .bind(indicators)
        .bind(signal.executed)
        .bind(signal.created_at)
        .bind(signal.expires_at)
        .execute(&self.pool)
        .await
        .context("insert signal")?;
        Ok(())
    }

    pub async fn mark_signal_executed(&self, signal_id: Uuid) -> Result<()> {
        sqlx::query("UPDATE signals SET executed = TRUE WHERE id = $1")
            .bind(signal_id)
            .execute(&self.pool)
            .await
            .context("mark signal executed")?;
        Ok(())
    }

    pub async fn insert_analysis(&self, analysis: &AnalysisResult) -> Result<()> {
        let model: String = analysis.grok_model_used.chars().take(64).collect();
        let tokens = i32::try_from(analysis.tokens_used).unwrap_or(i32::MAX);
        sqlx::query(
            r#"
            INSERT INTO analysis_logs (
                id, symbol, exchange, technical_summary, sentiment_summary,
                pattern_summary, overall_analysis, signal_id, grok_model, tokens_used, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
        )
        .bind(analysis.id)
        .bind(&analysis.symbol)
        .bind(analysis.exchange.as_db_str())
        .bind(&analysis.technical_summary)
        .bind(&analysis.sentiment_summary)
        .bind(&analysis.pattern_summary)
        .bind(&analysis.overall_analysis)
        .bind(analysis.signal.id)
        .bind(model)
        .bind(tokens)
        .bind(analysis.timestamp)
        .execute(&self.pool)
        .await
        .context("insert analysis log")?;
        Ok(())
    }

    pub async fn insert_trade(&self, trade: &TradeInsert) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO trades (
                id, exchange, symbol, side, quantity, entry_price, fees,
                signal_id, signal_confidence, opened_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(trade.id)
        .bind(trade.exchange.as_db_str())
        .bind(&trade.symbol)
        .bind(side_db(trade.side))
        .bind(trade.quantity)
        .bind(trade.entry_price)
        .bind(trade.fees)
        .bind(trade.signal_id)
        .bind(trade.signal_confidence)
        .bind(trade.opened_at)
        .execute(&self.pool)
        .await
        .context("insert trade")?;
        Ok(())
    }

    /// Fill the latest open trade for this symbol. Returns true when a row was updated.
    pub async fn close_open_trade(
        &self,
        exchange: ExchangeId,
        symbol: &str,
        signal_id: Option<Uuid>,
        exit_price: Decimal,
        fees: Decimal,
        realized_pnl: Decimal,
        close_reason: &str,
        closed_at: DateTime<Utc>,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE trades SET
                exit_price = $4,
                fees = $5,
                realized_pnl = $6,
                closed_at = $7,
                close_reason = $8
            WHERE id = (
                SELECT id FROM trades
                WHERE closed_at IS NULL
                  AND exchange = $1
                  AND symbol = $2
                  AND ($3::uuid IS NULL OR signal_id = $3)
                ORDER BY opened_at DESC
                LIMIT 1
            )
            "#,
        )
        .bind(exchange.as_db_str())
        .bind(symbol)
        .bind(signal_id)
        .bind(exit_price)
        .bind(fees)
        .bind(realized_pnl)
        .bind(closed_at)
        .bind(close_reason)
        .execute(&self.pool)
        .await
        .context("close trade")?;
        Ok(result.rows_affected() > 0)
    }

    /// Active rules for this symbol, plus rules scoped to every symbol.
    pub async fn active_playbook(&self, symbol: &str) -> Result<Vec<PlaybookRule>> {
        let symbol = normalize_scope(symbol);
        let rows = sqlx::query(
            r#"
            SELECT scope, rule, sample_size
            FROM playbook
            WHERE active = TRUE AND (scope = 'global' OR scope = $1)
            ORDER BY created_at ASC
            LIMIT 12
            "#,
        )
        .bind(&symbol)
        .fetch_all(&self.pool)
        .await
        .context("load playbook")?;

        let rules = rows
            .iter()
            .map(|row| {
                Ok(PlaybookRule {
                    scope: row.try_get("scope")?,
                    rule: row.try_get("rule")?,
                    sample_size: row.try_get("sample_size")?,
                })
            })
            .collect::<Result<Vec<_>, sqlx::Error>>()?;
        Ok(rules)
    }

    pub async fn list_playbook(&self) -> Result<Vec<PlaybookEntry>> {
        let rows = sqlx::query(
            r#"
            SELECT id, lesson_id, scope, rule, sample_size, created_at
            FROM playbook
            WHERE active = TRUE
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("list playbook")?;

        let rules = rows
            .iter()
            .map(|row| {
                Ok(PlaybookEntry {
                    id: row.try_get("id")?,
                    lesson_id: row.try_get("lesson_id")?,
                    scope: row.try_get("scope")?,
                    rule: row.try_get("rule")?,
                    sample_size: row.try_get("sample_size")?,
                    created_at: row.try_get("created_at")?,
                })
            })
            .collect::<Result<Vec<_>, sqlx::Error>>()?;
        Ok(rules)
    }

    /// Approve a rule. The lesson is stored as supported and the playbook row is active.
    pub async fn approve_playbook_rule(
        &self,
        scope: &str,
        rule: &str,
        sample_size: i32,
        evidence_trade_ids: &[Uuid],
    ) -> Result<(Uuid, Uuid)> {
        let scope = normalize_scope(scope);
        let mut tx = self.pool.begin().await.context("begin playbook transaction")?;

        let lesson_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO lessons (scope, claim, evidence_trade_ids, sample_size, status)
            VALUES ($1, $2, $3, $4, 'supported')
            RETURNING id
            "#,
        )
        .bind(&scope)
        .bind(rule)
        .bind(evidence_trade_ids)
        .bind(sample_size)
        .fetch_one(&mut *tx)
        .await
        .context("insert lesson")?;

        let playbook_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO playbook (lesson_id, scope, rule, sample_size, active)
            VALUES ($1, $2, $3, $4, TRUE)
            RETURNING id
            "#,
        )
        .bind(lesson_id)
        .bind(&scope)
        .bind(rule)
        .bind(sample_size)
        .fetch_one(&mut *tx)
        .await
        .context("insert playbook rule")?;

        tx.commit().await.context("commit playbook rule")?;
        Ok((lesson_id, playbook_id))
    }

    pub async fn list_trades(
        &self,
        symbol: Option<&str>,
        from: Option<DateTime<Utc>>,
        to: Option<DateTime<Utc>>,
        only_closed: bool,
        limit: i64,
    ) -> Result<Vec<TradeRow>> {
        let symbol = symbol.map(|value| value.trim().to_uppercase()).filter(|value| !value.is_empty());
        let rows = sqlx::query(
            r#"
            SELECT id, exchange, symbol, side, quantity, entry_price, exit_price, fees,
                   realized_pnl, signal_id, signal_confidence, opened_at, closed_at, close_reason
            FROM trades
            WHERE ($1::text IS NULL OR upper(symbol) = $1)
              AND ($2::timestamptz IS NULL OR COALESCE(closed_at, opened_at) >= $2)
              AND ($3::timestamptz IS NULL OR COALESCE(closed_at, opened_at) < $3)
              AND (NOT $4 OR closed_at IS NOT NULL)
            ORDER BY COALESCE(closed_at, opened_at) DESC
            LIMIT $5
            "#,
        )
        .bind(symbol)
        .bind(from)
        .bind(to)
        .bind(only_closed)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("list trades")?;

        rows.iter().map(read_trade_row).collect::<Result<Vec<_>, sqlx::Error>>().map_err(Into::into)
    }

    pub async fn get_trade(&self, id: Uuid) -> Result<Option<TradeDetail>> {
        let row = sqlx::query(
            r#"
            SELECT t.id, t.exchange, t.symbol, t.side, t.quantity, t.entry_price, t.exit_price,
                   t.fees, t.realized_pnl, t.signal_id, t.signal_confidence, t.opened_at,
                   t.closed_at, t.close_reason,
                   s.action AS signal_action,
                   s.reasoning AS signal_reasoning,
                   s.timeframe AS signal_timeframe,
                   s.indicators AS indicators
            FROM trades t
            LEFT JOIN signals s ON s.id = t.signal_id
            WHERE t.id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .context("get trade")?;

        let Some(row) = row else {
            return Ok(None);
        };
        let trade = read_trade_row(&row)?;
        Ok(Some(TradeDetail {
            trade,
            signal_action: row.try_get("signal_action")?,
            signal_reasoning: row.try_get("signal_reasoning")?,
            signal_timeframe: row.try_get("signal_timeframe")?,
            indicators: row.try_get("indicators")?,
        }))
    }

    pub async fn trade_performance(
        &self,
        symbol: Option<&str>,
        from: Option<DateTime<Utc>>,
        to: Option<DateTime<Utc>>,
    ) -> Result<PerformanceReport> {
        let symbol = symbol.map(|value| value.trim().to_uppercase()).filter(|value| !value.is_empty());
        let totals = sqlx::query(
            r#"
            SELECT
                COUNT(*)::bigint AS trades,
                COUNT(*) FILTER (WHERE realized_pnl > 0)::bigint AS wins,
                COUNT(*) FILTER (WHERE realized_pnl < 0)::bigint AS losses,
                COALESCE(SUM(realized_pnl), 0) AS total_pnl,
                COALESCE(SUM(realized_pnl) FILTER (WHERE realized_pnl > 0), 0) AS gross_profit,
                COALESCE(SUM(realized_pnl) FILTER (WHERE realized_pnl < 0), 0) AS gross_loss
            FROM trades
            WHERE closed_at IS NOT NULL
              AND ($1::text IS NULL OR upper(symbol) = $1)
              AND ($2::timestamptz IS NULL OR closed_at >= $2)
              AND ($3::timestamptz IS NULL OR closed_at < $3)
            "#,
        )
        .bind(&symbol)
        .bind(from)
        .bind(to)
        .fetch_one(&self.pool)
        .await
        .context("trade performance")?;

        let by_symbol = if symbol.is_some() {
            Vec::new()
        } else {
            let rows = sqlx::query(
                r#"
                SELECT
                    symbol,
                    COUNT(*)::bigint AS trades,
                    COUNT(*) FILTER (WHERE realized_pnl > 0)::bigint AS wins,
                    COUNT(*) FILTER (WHERE realized_pnl < 0)::bigint AS losses,
                    COALESCE(SUM(realized_pnl), 0) AS total_pnl,
                    COALESCE(SUM(realized_pnl) FILTER (WHERE realized_pnl > 0), 0) AS gross_profit,
                    COALESCE(SUM(realized_pnl) FILTER (WHERE realized_pnl < 0), 0) AS gross_loss
                FROM trades
                WHERE closed_at IS NOT NULL
                  AND ($1::timestamptz IS NULL OR closed_at >= $1)
                  AND ($2::timestamptz IS NULL OR closed_at < $2)
                GROUP BY symbol
                ORDER BY COUNT(*) DESC
                LIMIT 15
                "#,
            )
            .bind(from)
            .bind(to)
            .fetch_all(&self.pool)
            .await
            .context("trade performance by symbol")?;

            rows.iter()
                .map(|row| {
                    let figures = figures_from_row(row)?;
                    Ok(SymbolPerformance {
                        symbol: row.try_get("symbol")?,
                        figures,
                    })
                })
                .collect::<Result<Vec<_>, sqlx::Error>>()?
        };

        Ok(PerformanceReport {
            figures: figures_from_row(&totals)?,
            by_symbol,
        })
    }

    pub async fn list_lessons(
        &self,
        scope: Option<&str>,
        status: Option<&str>,
        limit: i64,
    ) -> Result<Vec<LessonRow>> {
        let scope = scope.map(normalize_scope).filter(|value| !value.is_empty());
        let rows = sqlx::query(
            r#"
            SELECT id, scope, claim, evidence_trade_ids, sample_size, status, created_at
            FROM lessons
            WHERE ($1::text IS NULL OR scope = $1 OR scope = 'global')
              AND ($2::text IS NULL OR status = $2)
            ORDER BY created_at DESC
            LIMIT $3
            "#,
        )
        .bind(scope)
        .bind(status)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("list lessons")?;

        rows.iter()
            .map(|row| {
                Ok(LessonRow {
                    id: row.try_get("id")?,
                    scope: row.try_get("scope")?,
                    claim: row.try_get("claim")?,
                    evidence_trade_ids: row.try_get("evidence_trade_ids")?,
                    sample_size: row.try_get("sample_size")?,
                    status: row.try_get("status")?,
                    created_at: row.try_get("created_at")?,
                })
            })
            .collect::<Result<Vec<_>, sqlx::Error>>()
            .map_err(Into::into)
    }

    /// Store a hypothesis. This does not add a playbook rule.
    pub async fn save_lesson(
        &self,
        scope: &str,
        claim: &str,
        sample_size: i32,
        evidence_trade_ids: &[Uuid],
    ) -> Result<Uuid> {
        let scope = normalize_scope(scope);
        sqlx::query_scalar(
            r#"
            INSERT INTO lessons (scope, claim, evidence_trade_ids, sample_size, status)
            VALUES ($1, $2, $3, $4, 'hypothesis')
            RETURNING id
            "#,
        )
        .bind(scope)
        .bind(claim)
        .bind(evidence_trade_ids)
        .bind(sample_size)
        .fetch_one(&self.pool)
        .await
        .context("insert lesson hypothesis")
    }

    pub async fn promote_lesson(&self, id: Uuid) -> Result<PromoteLesson> {
        let mut tx = self.pool.begin().await.context("begin lesson promotion")?;
        let row = sqlx::query(
            r#"
            SELECT scope, claim, sample_size, status
            FROM lessons
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .context("load lesson")?;

        let Some(row) = row else {
            return Ok(PromoteLesson::Missing);
        };
        let status: String = row.try_get("status")?;
        if status == "rejected" {
            return Ok(PromoteLesson::Rejected);
        }
        let scope: String = row.try_get("scope")?;
        let rule: String = row.try_get("claim")?;
        let sample_size: i32 = row.try_get("sample_size")?;

        sqlx::query("UPDATE lessons SET status = 'supported', updated_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .context("mark lesson supported")?;

        let existing: Option<Uuid> = sqlx::query_scalar(
            r#"
            UPDATE playbook
            SET active = TRUE, scope = $2, rule = $3, sample_size = $4
            WHERE id = (
                SELECT id FROM playbook WHERE lesson_id = $1 ORDER BY created_at DESC LIMIT 1
            )
            RETURNING id
            "#,
        )
        .bind(id)
        .bind(&scope)
        .bind(&rule)
        .bind(sample_size)
        .fetch_optional(&mut *tx)
        .await
        .context("reactivate playbook rule")?;

        let playbook_id = if let Some(playbook_id) = existing {
            playbook_id
        } else {
            sqlx::query_scalar(
                r#"
                INSERT INTO playbook (lesson_id, scope, rule, sample_size, active)
                VALUES ($1, $2, $3, $4, TRUE)
                RETURNING id
                "#,
            )
            .bind(id)
            .bind(&scope)
            .bind(&rule)
            .bind(sample_size)
            .fetch_one(&mut *tx)
            .await
            .context("insert playbook rule from lesson")?
        };

        tx.commit().await.context("commit lesson promotion")?;
        Ok(PromoteLesson::Promoted(PromotedLesson {
            lesson_id: id,
            playbook_id,
            scope,
            rule,
            sample_size,
        }))
    }

    pub async fn reject_lesson(&self, id: Uuid) -> Result<bool> {
        let mut tx = self.pool.begin().await.context("begin lesson rejection")?;
        let updated = sqlx::query(
            "UPDATE lessons SET status = 'rejected', updated_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .execute(&mut *tx)
        .await
        .context("reject lesson")?;
        if updated.rows_affected() == 0 {
            return Ok(false);
        }
        sqlx::query("UPDATE playbook SET active = FALSE WHERE lesson_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .context("deactivate rejected playbook rule")?;
        tx.commit().await.context("commit lesson rejection")?;
        Ok(true)
    }

    pub async fn recent_watched_posts(&self, limit: i64) -> Result<Vec<WatchedPost>> {
        let rows = sqlx::query(
            r#"
            SELECT post_id, handle, body, queued, seen_at
            FROM watched_posts
            ORDER BY seen_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("list watched posts")?;
        rows.iter()
            .map(|row| {
                Ok(WatchedPost {
                    post_id: row.try_get("post_id")?,
                    handle: row.try_get("handle")?,
                    body: row.try_get("body")?,
                    queued: row.try_get("queued")?,
                    seen_at: row.try_get("seen_at")?,
                })
            })
            .collect::<Result<Vec<_>, sqlx::Error>>()
            .map_err(Into::into)
    }

    pub async fn has_seen_post(&self, post_id: &str) -> Result<bool> {
        let found: Option<String> = sqlx::query_scalar(
            "SELECT post_id FROM watched_posts WHERE post_id = $1",
        )
        .bind(post_id)
        .fetch_optional(&self.pool)
        .await
        .context("lookup watched post")?;
        Ok(found.is_some())
    }

    pub async fn remember_post(
        &self,
        post_id: &str,
        handle: &str,
        body: &str,
        queued: bool,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO watched_posts (post_id, handle, body, queued)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (post_id) DO NOTHING
            "#,
        )
        .bind(post_id)
        .bind(handle)
        .bind(body)
        .bind(queued)
        .execute(&self.pool)
        .await
        .context("remember watched post")?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct TradeRow {
    pub id: Uuid,
    pub exchange: String,
    pub symbol: String,
    pub side: String,
    pub quantity: Decimal,
    pub entry_price: Decimal,
    pub exit_price: Option<Decimal>,
    pub fees: Decimal,
    pub realized_pnl: Option<Decimal>,
    pub signal_id: Option<Uuid>,
    pub signal_confidence: Option<f64>,
    pub opened_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub close_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TradeDetail {
    pub trade: TradeRow,
    pub signal_action: Option<String>,
    pub signal_reasoning: Option<String>,
    pub signal_timeframe: Option<String>,
    pub indicators: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct PerformanceFigures {
    pub trades: i64,
    pub wins: i64,
    pub losses: i64,
    pub win_rate_pct: f64,
    pub total_pnl: Decimal,
    pub average_win: Decimal,
    pub average_loss: Decimal,
    pub profit_factor: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct SymbolPerformance {
    pub symbol: String,
    pub figures: PerformanceFigures,
}

#[derive(Debug, Clone)]
pub struct PerformanceReport {
    pub figures: PerformanceFigures,
    pub by_symbol: Vec<SymbolPerformance>,
}

#[derive(Debug, Clone)]
pub struct PromotedLesson {
    pub lesson_id: Uuid,
    pub playbook_id: Uuid,
    pub scope: String,
    pub rule: String,
    pub sample_size: i32,
}

#[derive(Debug, Clone)]
pub enum PromoteLesson {
    Missing,
    Rejected,
    Promoted(PromotedLesson),
}

#[derive(Debug, Clone)]
pub struct WatchedPost {
    pub post_id: String,
    pub handle: String,
    pub body: String,
    pub queued: bool,
    pub seen_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct LessonRow {
    pub id: Uuid,
    pub scope: String,
    pub claim: String,
    pub evidence_trade_ids: Vec<Uuid>,
    pub sample_size: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

pub fn performance_figures(
    trades: i64,
    wins: i64,
    losses: i64,
    total_pnl: Decimal,
    gross_profit: Decimal,
    gross_loss: Decimal,
) -> PerformanceFigures {
    let win_rate_pct = if trades > 0 {
        (wins as f64 / trades as f64) * 100.0
    } else {
        0.0
    };
    PerformanceFigures {
        trades,
        wins,
        losses,
        win_rate_pct,
        total_pnl,
        average_win: average_decimal(gross_profit, wins),
        average_loss: average_decimal(gross_loss, losses),
        profit_factor: profit_factor(gross_profit, gross_loss),
    }
}

pub fn profit_factor(gross_profit: Decimal, gross_loss: Decimal) -> Option<f64> {
    let loss = gross_loss.abs();
    if loss.is_zero() {
        None
    } else {
        (gross_profit / loss).to_f64()
    }
}

fn average_decimal(sum: Decimal, count: i64) -> Decimal {
    if count <= 0 {
        Decimal::ZERO
    } else {
        sum / Decimal::from(count)
    }
}

fn figures_from_row(row: &sqlx::postgres::PgRow) -> Result<PerformanceFigures, sqlx::Error> {
    Ok(performance_figures(
        row.try_get("trades")?,
        row.try_get("wins")?,
        row.try_get("losses")?,
        row.try_get("total_pnl")?,
        row.try_get("gross_profit")?,
        row.try_get("gross_loss")?,
    ))
}

fn read_trade_row(row: &sqlx::postgres::PgRow) -> Result<TradeRow, sqlx::Error> {
    Ok(TradeRow {
        id: row.try_get("id")?,
        exchange: row.try_get("exchange")?,
        symbol: row.try_get("symbol")?,
        side: row.try_get("side")?,
        quantity: row.try_get("quantity")?,
        entry_price: row.try_get("entry_price")?,
        exit_price: row.try_get("exit_price")?,
        fees: row.try_get("fees")?,
        realized_pnl: row.try_get("realized_pnl")?,
        signal_id: row.try_get("signal_id")?,
        signal_confidence: row.try_get("signal_confidence")?,
        opened_at: row.try_get("opened_at")?,
        closed_at: row.try_get("closed_at")?,
        close_reason: row.try_get("close_reason")?,
    })
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn action_db(action: SignalAction) -> &'static str {
    match action {
        SignalAction::StrongBuy => "strong_buy",
        SignalAction::Buy => "buy",
        SignalAction::Hold => "hold",
        SignalAction::Sell => "sell",
        SignalAction::StrongSell => "strong_sell",
    }
}

fn source_db(source: SignalSource) -> &'static str {
    match source {
        SignalSource::Technical => "technical",
        SignalSource::Sentiment => "sentiment",
        SignalSource::Pattern => "pattern",
        SignalSource::AiDecision => "ai_decision",
        SignalSource::Combined => "combined",
        SignalSource::Manual => "manual",
    }
}

fn side_db(side: OrderSide) -> &'static str {
    match side {
        OrderSide::Buy => "buy",
        OrderSide::Sell => "sell",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_playbook_adds_nothing() {
        assert_eq!(format_playbook(&[]), "");
    }

    #[test]
    fn playbook_lists_scope_and_sample() {
        let text = format_playbook(&[PlaybookRule {
            scope: "BTCUSDT".to_string(),
            rule: "Skip buys when RSI is above 70".to_string(),
            sample_size: 34,
        }]);
        assert!(text.contains("ACTIVE PLAYBOOK"));
        assert!(text.contains("[BTCUSDT] Skip buys when RSI is above 70 (sample 34)"));
    }

    #[test]
    fn scope_normalizes_global_and_symbols() {
        assert_eq!(normalize_scope(" Global "), "global");
        assert_eq!(normalize_scope("btcusdt"), "BTCUSDT");
    }

    #[test]
    fn profit_factor_uses_absolute_loss() {
        let figures = performance_figures(
            3,
            2,
            1,
            Decimal::from(100),
            Decimal::from(200),
            Decimal::from(-100),
        );
        assert!((figures.win_rate_pct - 66.666).abs() < 0.01);
        assert_eq!(figures.profit_factor, Some(2.0));
        assert_eq!(figures.average_win, Decimal::from(100));
        assert_eq!(figures.average_loss, Decimal::from(-100));
    }

    #[test]
    fn profit_factor_is_absent_without_losses() {
        assert_eq!(profit_factor(Decimal::from(50), Decimal::ZERO), None);
    }
}

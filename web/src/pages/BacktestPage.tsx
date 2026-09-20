import { useEffect, useState } from 'react';
import { FlaskConical, Play, Sparkles, Zap, Bot, Cpu } from 'lucide-react';
import { StatCard } from '../components/StatCard';
import { LoadingSpinner } from '../components/LoadingSpinner';
import { useTradingStore } from '../stores/tradingStore';
import { api, type BacktestConfig, type BacktestResult } from '../api/client';
import './BacktestPage.css';

export function BacktestPage() {
  const { backtestResults, loading, fetchBacktestResults, runBacktest } = useTradingStore();

  const [config, setConfig] = useState<BacktestConfig>({
    symbol: 'BTCUSDT',
    timeframe: '1h',
    start_time: '2024-01-01T00:00:00Z',
    end_time: '2024-06-01T00:00:00Z',
    initial_capital: 10000,
    fee_rate: 0.001,
    stop_loss_pct: 2.0,
    take_profit_pct: 4.0,
    use_grok: false,
    max_ai_calls: 50,
  });

  const [selectedResultId, setSelectedResultId] = useState<string | null>(null);
  const [activeDetail, setActiveDetail] = useState<BacktestResult | null>(null);
  const [loadingDetail, setLoadingDetail] = useState(false);
  const [expandedTradeIdx, setExpandedTradeIdx] = useState<number | null>(null);

  useEffect(() => {
    fetchBacktestResults();
  }, []);

  const handleRun = async () => {
    const result = await runBacktest(config);
    if (result) {
      setSelectedResultId(result.id);
      setActiveDetail(result);
    }
  };

  const handleSelectResult = async (id: string) => {
    setSelectedResultId(id);
    setLoadingDetail(true);
    try {
      const res = await api.getBacktestResult(id);
      if (res && res.result) {
        setActiveDetail(res.result);
      }
    } catch (err) {
      console.error('Failed to load backtest details', err);
    } finally {
      setLoadingDetail(false);
    }
  };

  // Fallback to active summary if detail is loading or not yet fetched
  const selectedSummary = backtestResults.find((r) => r.id === selectedResultId);
  const displayResult = activeDetail && activeDetail.id === selectedResultId ? activeDetail : null;

  return (
    <div className="page backtest-page animate-fade-in">
      <div className="page__header">
        <div className="page__title-group">
          <h1 className="page__title">Backtesting Engine</h1>
          <p className="page__subtitle">
            Validate trading models against historical tick and candlestick data with optional Grok AI reasoning
          </p>
        </div>
      </div>

      <div className="backtest-page__layout">
        {/* Config Panel */}
        <div className="backtest-page__config card">
          <h2 className="card__title">Engine & Parameters</h2>

          <div className="backtest-page__form">
            {/* AI Engine Mode Toggle */}
            <div className="backtest-page__field">
              <label className="label">Decision Engine</label>
              <div className="backtest-mode-toggle">
                <button
                  id="toggle-fast-quant"
                  type="button"
                  className={`backtest-mode-btn ${!config.use_grok ? 'active' : ''}`}
                  onClick={() => setConfig({ ...config, use_grok: false })}
                >
                  <Zap size={16} />
                  <div>
                    <strong>Fast Quant</strong>
                    <span>Instant • $0.00</span>
                  </div>
                </button>

                <button
                  id="toggle-grok-ai"
                  type="button"
                  className={`backtest-mode-btn ${config.use_grok ? 'active grok-mode' : ''}`}
                  onClick={() => setConfig({ ...config, use_grok: true })}
                >
                  <Sparkles size={16} />
                  <div>
                    <strong>Grok AI Evaluated</strong>
                    <span>xAI LLM • Exact Live Logic</span>
                  </div>
                </button>
              </div>
            </div>

            {/* Grok AI Budget Controls */}
            {config.use_grok && (
              <div className="backtest-grok-config animate-fade-in">
                <div className="backtest-grok-config__header">
                  <span className="badge badge-purple">
                    <Bot size={13} style={{ marginRight: 4 }} /> Grok-4.6 Filter Gate
                  </span>
                  <span className="text-xs text-muted">
                    Est. &lt; ${(config.max_ai_calls || 50) * 0.005} / run
                  </span>
                </div>
                <div className="backtest-page__field" style={{ marginTop: 'var(--space-2)' }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                    <label className="label">AI Candidate Budget Cap</label>
                    <span className="text-sm text-mono" style={{ color: 'var(--accent-purple)' }}>
                      {config.max_ai_calls} calls max
                    </span>
                  </div>
                  <input
                    type="range"
                    min="10"
                    max="150"
                    step="10"
                    value={config.max_ai_calls || 50}
                    onChange={(e) => setConfig({ ...config, max_ai_calls: parseInt(e.target.value) })}
                    className="backtest-range-slider"
                  />
                  <p className="text-xs text-muted" style={{ marginTop: '4px' }}>
                    Only high-volatility/pattern setups query Grok. All queries are cached for free subsequent runs.
                  </p>
                </div>
              </div>
            )}

            <div className="backtest-page__field">
              <label className="label">Symbol</label>
              <select className="input" value={config.symbol} onChange={(e) => setConfig({ ...config, symbol: e.target.value })}>
                <option value="BTCUSDT">BTCUSDT</option>
                <option value="ETHUSDT">ETHUSDT</option>
                <option value="SOLUSDT">SOLUSDT</option>
                <option value="BNBUSDT">BNBUSDT</option>
              </select>
            </div>

            <div className="backtest-page__field">
              <label className="label">Timeframe</label>
              <select className="input" value={config.timeframe} onChange={(e) => setConfig({ ...config, timeframe: e.target.value })}>
                <option value="1m">1 Minute</option>
                <option value="5m">5 Minutes</option>
                <option value="15m">15 Minutes</option>
                <option value="1h">1 Hour</option>
                <option value="4h">4 Hours</option>
                <option value="1d">1 Day</option>
              </select>
            </div>

            <div className="backtest-page__row">
              <div className="backtest-page__field">
                <label className="label">Start Date</label>
                <input
                  type="date"
                  className="input"
                  value={config.start_time.slice(0, 10)}
                  onChange={(e) => setConfig({ ...config, start_time: `${e.target.value}T00:00:00Z` })}
                />
              </div>
              <div className="backtest-page__field">
                <label className="label">End Date</label>
                <input
                  type="date"
                  className="input"
                  value={config.end_time.slice(0, 10)}
                  onChange={(e) => setConfig({ ...config, end_time: `${e.target.value}T00:00:00Z` })}
                />
              </div>
            </div>

            <div className="backtest-page__field">
              <label className="label">Initial Capital (USD)</label>
              <input
                type="number"
                className="input"
                value={config.initial_capital}
                onChange={(e) => setConfig({ ...config, initial_capital: parseFloat(e.target.value) || 1000 })}
              />
            </div>

            <div className="backtest-page__row">
              <div className="backtest-page__field">
                <label className="label">Stop Loss %</label>
                <input
                  type="number"
                  className="input"
                  step="0.1"
                  value={config.stop_loss_pct}
                  onChange={(e) => setConfig({ ...config, stop_loss_pct: parseFloat(e.target.value) || 1.0 })}
                />
              </div>
              <div className="backtest-page__field">
                <label className="label">Take Profit %</label>
                <input
                  type="number"
                  className="input"
                  step="0.1"
                  value={config.take_profit_pct}
                  onChange={(e) => setConfig({ ...config, take_profit_pct: parseFloat(e.target.value) || 2.0 })}
                />
              </div>
            </div>

            <div className="backtest-page__field">
              <label className="label">Fee Rate</label>
              <input
                type="number"
                className="input"
                step="0.0001"
                value={config.fee_rate}
                onChange={(e) => setConfig({ ...config, fee_rate: parseFloat(e.target.value) || 0.001 })}
              />
            </div>

            <button
              id="btn-run-backtest-panel"
              className={`btn btn-lg w-full ${config.use_grok ? 'btn-grok-run' : 'btn-primary'}`}
              onClick={handleRun}
              disabled={loading.backtest_run}
            >
              {loading.backtest_run ? (
                <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                  <LoadingSpinner size={18} />
                  <span>{config.use_grok ? 'Evaluating with Grok AI...' : 'Computing Backtest...'}</span>
                </div>
              ) : (
                <>
                  {config.use_grok ? <Sparkles size={18} /> : <Play size={18} />}
                  <span>{config.use_grok ? 'Run Grok AI Backtest' : 'Run Fast Backtest'}</span>
                </>
              )}
            </button>
          </div>
        </div>

        {/* Results Panel */}
        <div className="backtest-page__results">
          {loadingDetail ? (
            <div className="backtest-page__empty card">
              <LoadingSpinner size={36} />
              <h3>Loading Simulation Details...</h3>
            </div>
          ) : (displayResult || selectedSummary) ? (
            <>
              {/* Header Info & Engine Badges */}
              <div className="card backtest-result-header">
                <div className="backtest-result-header__title">
                  <h2>
                    {displayResult?.config?.symbol || selectedSummary?.config?.symbol || (selectedSummary as any)?.symbol || 'BTCUSDT'} • {displayResult?.config?.timeframe || selectedSummary?.config?.timeframe || (selectedSummary as any)?.timeframe || '1h'}
                  </h2>
                  <span className="text-sm text-muted">
                    Completed at {displayResult?.completed_at || selectedSummary?.completed_at ? new Date((displayResult?.completed_at || selectedSummary?.completed_at)!).toLocaleString() : 'Recently'}
                  </span>
                </div>

                <div className="backtest-result-badges">
                  {(displayResult?.use_grok ?? selectedSummary?.use_grok) ? (
                    <div className="grok-pill-badge">
                      <Sparkles size={14} className="text-purple" />
                      <span>Grok AI Evaluated</span>
                      <span className="grok-pill-divider">|</span>
                      <span className="text-mono">
                        {(displayResult?.ai_calls_made ?? selectedSummary?.ai_calls_made ?? 0)} calls
                        {(displayResult?.ai_cached_calls ?? selectedSummary?.ai_cached_calls ?? 0) > 0 && (
                          <span className="text-muted"> ({(displayResult?.ai_cached_calls ?? selectedSummary?.ai_cached_calls)} cached)</span>
                        )}
                      </span>
                    </div>
                  ) : (
                    <div className="quant-pill-badge">
                      <Zap size={14} />
                      <span>Fast Quantitative Engine</span>
                    </div>
                  )}
                </div>
              </div>

              {/* Stat Cards */}
              {(() => {
                const metrics = displayResult?.metrics || selectedSummary?.metrics || (selectedSummary as any);
                if (!metrics) return null;
                const totalReturn = metrics.total_return_pct ?? 0;
                const winRate = metrics.win_rate_pct ?? 0;
                const sharpe = metrics.sharpe_ratio ?? 0;
                const maxDd = metrics.max_drawdown_pct ?? 0;
                const totalTrades = metrics.total_trades ?? 0;
                const profitFactor = metrics.profit_factor ?? 0;

                return (
                  <div className="grid grid-3 gap-4" style={{ marginTop: 'var(--space-4)' }}>
                    <StatCard
                      label="Total Return"
                      value={`${totalReturn >= 0 ? '+' : ''}${totalReturn.toFixed(2)}`}
                      suffix="%"
                      variant={totalReturn >= 0 ? 'profit' : 'loss'}
                    />
                    <StatCard label="Win Rate" value={`${winRate.toFixed(1)}`} suffix="%" />
                    <StatCard label="Sharpe Ratio" value={sharpe.toFixed(2)} />
                    <StatCard label="Max Drawdown" value={`${maxDd.toFixed(2)}`} suffix="%" variant="loss" />
                    <StatCard label="Total Trades" value={totalTrades} />
                    <StatCard label="Profit Factor" value={profitFactor.toFixed(2)} />
                  </div>
                );
              })()}

              {/* Trade Execution & Grok Reasoning Log */}
              {displayResult && displayResult.trades && displayResult.trades.length > 0 && (
                <div className="card" style={{ marginTop: 'var(--space-4)' }}>
                  <div className="card__header" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 'var(--space-3)' }}>
                    <h2 className="card__title" style={{ margin: 0, display: 'flex', alignItems: 'center', gap: '8px' }}>
                      <Cpu size={18} /> Executed Trades & AI Decisions ({displayResult.trades.length})
                    </h2>
                    <span className="text-xs text-muted">Click a trade row to inspect entry reasoning</span>
                  </div>

                  <div className="table-container">
                    <table className="backtest-trades-table">
                      <thead>
                        <tr>
                          <th>#</th>
                          <th>Entry Time</th>
                          <th>Side</th>
                          <th className="text-right">Entry Price</th>
                          <th className="text-right">Exit Price</th>
                          <th className="text-right">PnL</th>
                          <th className="text-right">Return</th>
                          <th>Exit Reason</th>
                          <th>Reasoning</th>
                        </tr>
                      </thead>
                      <tbody>
                        {displayResult.trades.map((trade, idx) => {
                          const isExpanded = expandedTradeIdx === idx;
                          const isWin = trade.pnl >= 0;
                          return (
                            <>
                              <tr
                                key={idx}
                                className={`backtest-trade-row ${isExpanded ? 'expanded' : ''}`}
                                onClick={() => setExpandedTradeIdx(isExpanded ? null : idx)}
                                style={{ cursor: 'pointer' }}
                              >
                                <td className="text-muted text-mono">{idx + 1}</td>
                                <td className="text-mono text-sm">{trade.entry_time.slice(0, 16).replace('T', ' ')}</td>
                                <td>
                                  <span className={`badge ${trade.side.toUpperCase() === 'BUY' || trade.side.toUpperCase() === 'LONG' ? 'badge-profit' : 'badge-loss'}`}>
                                    {trade.side.toUpperCase()}
                                  </span>
                                </td>
                                <td className="text-right text-mono">${trade.entry_price.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}</td>
                                <td className="text-right text-mono">${trade.exit_price.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}</td>
                                <td className={`text-right text-mono ${isWin ? 'text-profit' : 'text-loss'}`}>
                                  {isWin ? '+' : ''}${trade.pnl.toFixed(2)}
                                </td>
                                <td className={`text-right text-mono font-bold ${isWin ? 'text-profit' : 'text-loss'}`}>
                                  {isWin ? '+' : ''}{trade.pnl_pct.toFixed(2)}%
                                </td>
                                <td>
                                  <span className="badge badge-gray">{trade.exit_reason || 'Signal'}</span>
                                </td>
                                <td style={{ maxWidth: '240px', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                                  <span className="text-xs text-muted" title={trade.reasoning}>
                                    {trade.reasoning ? trade.reasoning.slice(0, 38) + '...' : 'Technical setup'}
                                  </span>
                                </td>
                              </tr>
                              {isExpanded && (
                                <tr className="backtest-trade-expansion-row">
                                  <td colSpan={9}>
                                    <div className="trade-expanded-card">
                                      <div className="trade-expanded-header">
                                        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                                          <Bot size={16} style={{ color: 'var(--accent-purple)' }} />
                                          <strong>Signal Reasoning & AI Context</strong>
                                        </div>
                                        <div className="text-xs text-muted">
                                          Holding: {trade.holding_bars || 1} bars • Fees: ${trade.fees?.toFixed(2) || '0.00'}
                                        </div>
                                      </div>
                                      <div className="trade-expanded-body">
                                        <p className="trade-reasoning-text">
                                          {trade.reasoning || 'Executed based on multi-indicator technical confluence.'}
                                        </p>
                                      </div>
                                    </div>
                                  </td>
                                </tr>
                              )}
                            </>
                          );
                        })}
                      </tbody>
                    </table>
                  </div>
                </div>
              )}
            </>
          ) : (
            <div className="backtest-page__empty card">
              <FlaskConical size={48} strokeWidth={1} />
              <h3>Run a Backtest</h3>
              <p>
                Configure parameters on the left and run either a free fast quantitative test or a full Grok AI evaluation.
              </p>
              <button
                id="btn-run-backtest-empty"
                className={`btn btn-lg ${config.use_grok ? 'btn-grok-run' : 'btn-primary'}`}
                style={{ marginTop: 'var(--space-3)' }}
                onClick={handleRun}
                disabled={loading.backtest_run}
              >
                {loading.backtest_run ? (
                  <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                    <LoadingSpinner size={18} />
                    <span>{config.use_grok ? 'Evaluating with Grok AI...' : 'Computing Backtest...'}</span>
                  </div>
                ) : (
                  <>
                    {config.use_grok ? <Sparkles size={18} /> : <Play size={18} />}
                    <span>{config.use_grok ? 'Run Grok AI Backtest' : 'Run Fast Backtest'}</span>
                  </>
                )}
              </button>
            </div>
          )}

          {/* Previous Results Table */}
          {backtestResults.length > 0 && (
            <div className="card" style={{ marginTop: 'var(--space-4)' }}>
              <h2 className="card__title">Previous Simulation Runs</h2>
              <div className="table-container">
                <table>
                  <thead>
                    <tr>
                      <th>Symbol</th>
                      <th>Timeframe</th>
                      <th>Engine</th>
                      <th className="text-right">Return</th>
                      <th className="text-right">Win Rate</th>
                      <th className="text-right">Sharpe</th>
                      <th>Trades</th>
                      <th>Action</th>
                    </tr>
                  </thead>
                  <tbody>
                    {backtestResults.map((r) => (
                      <tr
                        key={r.id}
                        className={selectedResultId === r.id ? 'backtest-page__selected-row' : ''}
                      >
                        <td><strong>{r.config?.symbol || (r as any).symbol || 'BTCUSDT'}</strong></td>
                        <td>{r.config?.timeframe || (r as any).timeframe || '1h'}</td>
                        <td>
                          {r.use_grok ? (
                            <span className="badge badge-purple" style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
                              <Sparkles size={11} /> Grok AI ({r.ai_calls_made || 0})
                            </span>
                          ) : (
                            <span className="badge badge-gray">Fast Quant</span>
                          )}
                        </td>
                        <td className={`text-right text-mono ${(r.metrics?.total_return_pct ?? (r as any).total_return_pct ?? 0) >= 0 ? 'text-profit' : 'text-loss'}`}>
                          {(r.metrics?.total_return_pct ?? (r as any).total_return_pct ?? 0) >= 0 ? '+' : ''}{(r.metrics?.total_return_pct ?? (r as any).total_return_pct ?? 0).toFixed(2)}%
                        </td>
                        <td className="text-right text-mono">{(r.metrics?.win_rate_pct ?? (r as any).win_rate_pct ?? 0).toFixed(1)}%</td>
                        <td className="text-right text-mono">{(r.metrics?.sharpe_ratio ?? (r as any).sharpe_ratio ?? 0).toFixed(2)}</td>
                        <td>{r.metrics?.total_trades ?? (r as any).total_trades ?? 0}</td>
                        <td>
                          <button
                            className="btn btn-ghost btn-sm"
                            onClick={() => handleSelectResult(r.id)}
                          >
                            View
                          </button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}


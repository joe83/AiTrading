import { useEffect, useState } from 'react';
import { FlaskConical, Play } from 'lucide-react';
import { StatCard } from '../components/StatCard';
import { LoadingSpinner } from '../components/LoadingSpinner';
import { useTradingStore } from '../stores/tradingStore';
import type { BacktestConfig } from '../api/client';
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
  });

  const [selectedResult, setSelectedResult] = useState<string | null>(null);

  useEffect(() => {
    fetchBacktestResults();
  }, []);

  const handleRun = () => {
    runBacktest(config);
  };

  const activeResult = backtestResults.find((r) => r.id === selectedResult);

  return (
    <div className="page backtest-page animate-fade-in">
      <div className="page__header">
        <h1 className="page__title">Backtesting</h1>
        <p className="page__subtitle">Test strategies against historical data</p>
      </div>

      <div className="backtest-page__layout">
        {/* Config Panel */}
        <div className="backtest-page__config card">
          <h2 className="card__title">Configuration</h2>

          <div className="backtest-page__form">
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
                onChange={(e) => setConfig({ ...config, initial_capital: parseFloat(e.target.value) })}
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
                  onChange={(e) => setConfig({ ...config, stop_loss_pct: parseFloat(e.target.value) })}
                />
              </div>
              <div className="backtest-page__field">
                <label className="label">Take Profit %</label>
                <input
                  type="number"
                  className="input"
                  step="0.1"
                  value={config.take_profit_pct}
                  onChange={(e) => setConfig({ ...config, take_profit_pct: parseFloat(e.target.value) })}
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
                onChange={(e) => setConfig({ ...config, fee_rate: parseFloat(e.target.value) })}
              />
            </div>

            <button className="btn btn-primary btn-lg w-full" onClick={handleRun} disabled={loading.backtest_run}>
              {loading.backtest_run ? (
                <LoadingSpinner size={18} />
              ) : (
                <>
                  <Play size={18} /> Run Backtest
                </>
              )}
            </button>
          </div>
        </div>

        {/* Results Panel */}
        <div className="backtest-page__results">
          {activeResult ? (
            <>
              <div className="grid grid-3 gap-4">
                <StatCard label="Total Return" value={`${activeResult.metrics.total_return_pct.toFixed(2)}`} suffix="%" variant={activeResult.metrics.total_return_pct >= 0 ? 'profit' : 'loss'} />
                <StatCard label="Sharpe Ratio" value={activeResult.metrics.sharpe_ratio.toFixed(2)} />
                <StatCard label="Max Drawdown" value={`${activeResult.metrics.max_drawdown_pct.toFixed(2)}`} suffix="%" variant="loss" />
                <StatCard label="Win Rate" value={`${activeResult.metrics.win_rate_pct.toFixed(1)}`} suffix="%" />
                <StatCard label="Total Trades" value={activeResult.metrics.total_trades} />
                <StatCard label="Profit Factor" value={activeResult.metrics.profit_factor.toFixed(2)} />
              </div>
            </>
          ) : (
            <div className="backtest-page__empty card">
              <FlaskConical size={48} strokeWidth={1} />
              <h3>Run a Backtest</h3>
              <p>Configure parameters and click "Run Backtest" to see results, or select a previous run from the list below.</p>
            </div>
          )}

          {/* Previous Results */}
          {backtestResults.length > 0 && (
            <div className="card" style={{ marginTop: 'var(--space-4)' }}>
              <h2 className="card__title">Previous Runs</h2>
              <div className="table-container">
                <table>
                  <thead>
                    <tr>
                      <th>Symbol</th>
                      <th>Timeframe</th>
                      <th className="text-right">Return</th>
                      <th className="text-right">Win Rate</th>
                      <th className="text-right">Sharpe</th>
                      <th>Trades</th>
                      <th></th>
                    </tr>
                  </thead>
                  <tbody>
                    {backtestResults.map((r) => (
                      <tr
                        key={r.id}
                        className={selectedResult === r.id ? 'backtest-page__selected-row' : ''}
                      >
                        <td><strong>{r.config.symbol}</strong></td>
                        <td>{r.config.timeframe}</td>
                        <td className={`text-right text-mono ${r.metrics.total_return_pct >= 0 ? 'text-profit' : 'text-loss'}`}>
                          {r.metrics.total_return_pct.toFixed(2)}%
                        </td>
                        <td className="text-right text-mono">{r.metrics.win_rate_pct.toFixed(1)}%</td>
                        <td className="text-right text-mono">{r.metrics.sharpe_ratio.toFixed(2)}</td>
                        <td>{r.metrics.total_trades}</td>
                        <td>
                          <button className="btn btn-ghost btn-sm" onClick={() => setSelectedResult(r.id)}>
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

import { useEffect } from 'react';
import { Activity, TrendingUp, Briefcase, Bot } from 'lucide-react';
import { StatCard } from '../components/StatCard';
import { PnlDisplay } from '../components/PnlDisplay';
import { StatusBadge } from '../components/StatusBadge';
import { ConfidenceBar } from '../components/ConfidenceBar';
import { LoadingSpinner } from '../components/LoadingSpinner';
import { useTradingStore } from '../stores/tradingStore';
import './DashboardPage.css';

export function DashboardPage() {
  const {
    dashboard,
    performance,
    signals,
    pendingSignals,
    loading,
    fetchDashboard,
    fetchPerformance,
    fetchSignals,
    fetchPendingSignals,
  } = useTradingStore();

  useEffect(() => {
    fetchDashboard();
    fetchPerformance();
    fetchSignals();
    fetchPendingSignals();
  }, []);

  if (loading.dashboard && !dashboard) {
    return <LoadingSpinner text="Loading dashboard..." />;
  }

  return (
    <div className="page dashboard-page animate-fade-in">
      <div className="page__header">
        <h1 className="page__title">Dashboard</h1>
        <p className="page__subtitle">Overview of your trading activity</p>
      </div>

      {/* Stats Row */}
      <div className="grid grid-4 gap-4">
        <StatCard
          label="Total P&L"
          value={dashboard?.unrealized_pnl?.toFixed(2) ?? '0.00'}
          prefix="$"
          change={dashboard?.unrealized_pnl ? (dashboard.unrealized_pnl > 0 ? 2.4 : -1.2) : 0}
          icon={<TrendingUp size={18} />}
          variant={dashboard?.unrealized_pnl && dashboard.unrealized_pnl > 0 ? 'profit' : dashboard?.unrealized_pnl && dashboard.unrealized_pnl < 0 ? 'loss' : 'default'}
        />
        <StatCard
          label="Win Rate"
          value={`${(dashboard?.win_rate ?? 0).toFixed(1)}`}
          suffix="%"
          icon={<Activity size={18} />}
        />
        <StatCard
          label="Open Positions"
          value={dashboard?.open_positions ?? 0}
          icon={<Briefcase size={18} />}
        />
        <StatCard
          label="Pending Signals"
          value={dashboard?.pending_signals ?? 0}
          icon={<Bot size={18} />}
        />
      </div>

      {/* Two-column layout */}
      <div className="dashboard-page__grid">
        {/* Recent Positions */}
        <div className="card">
          <h2 className="card__title">Active Positions</h2>
          {dashboard?.positions && dashboard.positions.length > 0 ? (
            <div className="table-container">
              <table>
                <thead>
                  <tr>
                    <th>Symbol</th>
                    <th>Side</th>
                    <th>Entry</th>
                    <th>P&L</th>
                  </tr>
                </thead>
                <tbody>
                  {dashboard.positions.slice(0, 5).map((pos, i) => (
                    <tr key={pos.id || i}>
                      <td>
                        <span className="dashboard-page__symbol">{pos.symbol}</span>
                        <span className="text-muted" style={{ fontSize: 'var(--text-xs)', marginLeft: '6px' }}>
                          {pos.exchange}
                        </span>
                      </td>
                      <td>
                        <StatusBadge
                          status={pos.side === 'buy' ? 'active' : 'error'}
                          label={pos.side.toUpperCase()}
                        />
                      </td>
                      <td className="text-mono">{parseFloat(pos.entry_price).toFixed(2)}</td>
                      <td><PnlDisplay value={pos.unrealized_pnl} /></td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : (
            <div className="dashboard-page__empty">
              <Briefcase size={32} strokeWidth={1} />
              <p>No open positions</p>
            </div>
          )}
        </div>

        {/* Recent Signals */}
        <div className="card">
          <h2 className="card__title">Recent AI Signals</h2>
          {(pendingSignals.length > 0 || signals.length > 0) ? (
            <div className="dashboard-page__signals">
              {[...pendingSignals, ...signals].slice(0, 5).map((sig, i) => (
                <div key={sig.id || i} className="dashboard-page__signal-item">
                  <div className="dashboard-page__signal-header">
                    <span className="dashboard-page__symbol">{sig.symbol}</span>
                    <StatusBadge
                      status={sig.action === 'BUY' ? 'active' : 'error'}
                      label={sig.action}
                    />
                  </div>
                  <ConfidenceBar value={sig.confidence} label="AI Confidence" size="sm" />
                  <p className="dashboard-page__signal-reason">{sig.reasoning?.slice(0, 100)}...</p>
                </div>
              ))}
            </div>
          ) : (
            <div className="dashboard-page__empty">
              <Bot size={32} strokeWidth={1} />
              <p>No recent signals</p>
            </div>
          )}
        </div>
      </div>

      {/* Performance Summary */}
      {performance && (
        <div className="card">
          <h2 className="card__title">Performance Summary</h2>
          <div className="grid grid-4 gap-4" style={{ marginTop: 'var(--space-4)' }}>
            <div className="dashboard-page__perf-item">
              <span className="label">Total Trades</span>
              <span className="dashboard-page__perf-value">{performance.total_trades}</span>
            </div>
            <div className="dashboard-page__perf-item">
              <span className="label">Wins</span>
              <span className="dashboard-page__perf-value text-profit">{performance.wins}</span>
            </div>
            <div className="dashboard-page__perf-item">
              <span className="label">Losses</span>
              <span className="dashboard-page__perf-value text-loss">{performance.losses}</span>
            </div>
            <div className="dashboard-page__perf-item">
              <span className="label">Win Rate</span>
              <span className="dashboard-page__perf-value">{performance.win_rate.toFixed(1)}%</span>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

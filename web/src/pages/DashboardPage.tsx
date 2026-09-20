import { useEffect } from 'react';
import { Link } from 'react-router-dom';
import { Activity, TrendingUp, Briefcase, Bot, Flame, ArrowRight, Sparkles } from 'lucide-react';
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

      {/* Meme Radar Quick Access Banner */}
      <div className="card" style={{
        background: 'linear-gradient(135deg, rgba(30, 27, 75, 0.4) 0%, rgba(245, 158, 11, 0.08) 100%)',
        borderColor: 'rgba(245, 158, 11, 0.3)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        padding: '16px 20px',
        flexWrap: 'wrap',
        gap: '12px',
      }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '14px' }}>
          <div style={{
            width: '36px',
            height: '36px',
            borderRadius: '8px',
            background: 'rgba(245, 158, 11, 0.15)',
            border: '1px solid rgba(245, 158, 11, 0.35)',
            color: '#fbbf24',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
          }}>
            <Flame size={20} />
          </div>
          <div>
            <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
              <span style={{ fontWeight: 700, fontSize: '0.9375rem', color: 'var(--text-primary)' }}>
                Meme Coin & Social Sentiment Radar
              </span>
              <span className="badge badge-info" style={{ fontSize: '0.6875rem' }}>
                <Sparkles size={11} /> Powered by Grok AI
              </span>
            </div>
            <p style={{ margin: 0, fontSize: '0.8125rem', color: 'var(--text-tertiary)', marginTop: '2px' }}>
              Track viral attention spikes on Crypto Twitter (X) and trade trending meme tokens on MEXC Spot.
            </p>
          </div>
        </div>
        <Link to="/meme-radar" className="btn btn-primary btn-sm" style={{
          background: 'linear-gradient(135deg, #f59e0b 0%, #ef4444 100%)',
          border: 'none',
          fontWeight: 700,
          display: 'inline-flex',
          alignItems: 'center',
          gap: '6px',
        }}>
          Open Meme Radar <ArrowRight size={14} />
        </Link>
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

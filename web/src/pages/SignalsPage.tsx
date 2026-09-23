import { useEffect } from 'react';
import { Bot, Check, X } from 'lucide-react';
import { ConfidenceBar } from '../components/ConfidenceBar';
import { StatusBadge } from '../components/StatusBadge';
import { useTradingStore } from '../stores/tradingStore';
import './SignalsPage.css';

export function SignalsPage() {
  const {
    signals,
    pendingSignals,
    fetchSignals,
    fetchPendingSignals,
    approveSignal,
    rejectSignal,
  } = useTradingStore();

  useEffect(() => {
    fetchSignals();
    fetchPendingSignals();
    const timer = window.setInterval(() => {
      fetchSignals();
      fetchPendingSignals();
    }, 15000);
    return () => window.clearInterval(timer);
  }, [fetchSignals, fetchPendingSignals]);

  return (
    <div className="page signals-page animate-fade-in">
      <div className="page__header">
        <h1 className="page__title">AI Signals</h1>
        <p className="page__subtitle">AI-generated trading signals from Grok analysis</p>
      </div>

      {/* Pending Signals */}
      <section className="signals-page__section">
        <h2 className="signals-page__section-title">
          Pending Approval
          {pendingSignals.length > 0 && (
            <span className="badge badge-warning">{pendingSignals.length}</span>
          )}
        </h2>

        {pendingSignals.length > 0 ? (
          <div className="signals-page__pending-grid">
            {pendingSignals.map((sig, i) => (
              <div key={sig.id || i} className="signals-page__signal-card card">
                <div className="signals-page__card-header">
                  <div>
                    <span className="signals-page__symbol">{sig.symbol}</span>
                    <span className="badge badge-info" style={{ marginLeft: '8px' }}>{sig.exchange}</span>
                  </div>
                  <StatusBadge
                    status={sig.action === 'BUY' ? 'active' : 'error'}
                    label={sig.action}
                  />
                </div>

                <ConfidenceBar value={sig.confidence} label="AI Confidence" />

                <p className="signals-page__reasoning">{sig.reasoning}</p>

                <div className="signals-page__card-actions">
                  <button className="btn btn-success" onClick={() => approveSignal(sig.id)}>
                    <Check size={16} /> Approve
                  </button>
                  <button className="btn btn-danger" onClick={() => rejectSignal(sig.id)}>
                    <X size={16} /> Reject
                  </button>
                </div>

                <span className="signals-page__time">
                  {new Date(sig.timestamp).toLocaleString()}
                </span>
              </div>
            ))}
          </div>
        ) : (
          <div className="signals-page__empty card">
            <Bot size={40} strokeWidth={1} />
            <p>No pending signals</p>
            <p className="text-muted" style={{ fontSize: 'var(--text-xs)' }}>
              Signals appear here when Grok AI identifies trading opportunities
            </p>
          </div>
        )}
      </section>

      {/* Signal History */}
      <section className="signals-page__section">
        <h2 className="signals-page__section-title">Signal History</h2>

        {signals.length > 0 ? (
          <div className="table-container">
            <table>
              <thead>
                <tr>
                  <th>Time</th>
                  <th>Symbol</th>
                  <th>Action</th>
                  <th>Confidence</th>
                  <th>Reasoning</th>
                  <th>Status</th>
                </tr>
              </thead>
              <tbody>
                {signals.map((sig, i) => (
                  <tr key={sig.id || i}>
                    <td className="text-muted" style={{ fontSize: 'var(--text-xs)' }}>
                      {new Date(sig.timestamp).toLocaleString()}
                    </td>
                    <td><span className="signals-page__symbol">{sig.symbol}</span></td>
                    <td>
                      <StatusBadge
                        status={sig.action === 'BUY' ? 'active' : 'error'}
                        label={sig.action}
                      />
                    </td>
                    <td style={{ minWidth: '120px' }}>
                      <ConfidenceBar value={sig.confidence} showValue={true} size="sm" />
                    </td>
                    <td style={{ maxWidth: '300px', whiteSpace: 'normal' }}>
                      <span className="signals-page__reasoning-cell">{sig.reasoning?.slice(0, 80)}...</span>
                    </td>
                    <td>
                      <StatusBadge status={sig.status === 'approved' ? 'active' : sig.status === 'rejected' ? 'error' : 'pending'} label={sig.status || 'processed'} />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <div className="signals-page__empty card">
            <p className="text-muted">No signal history yet</p>
          </div>
        )}
      </section>
    </div>
  );
}

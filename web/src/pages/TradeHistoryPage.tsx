import { useEffect } from 'react';
import { ScrollText } from 'lucide-react';
import { PnlDisplay } from '../components/PnlDisplay';
import { StatusBadge } from '../components/StatusBadge';
import { useTradingStore } from '../stores/tradingStore';
import './TradeHistoryPage.css';

export function TradeHistoryPage() {
  const { trades, fetchTrades } = useTradingStore();

  useEffect(() => {
    fetchTrades();
  }, []);

  const closedTrades = trades.filter((t) => t.closed_at);
  const totalPnl = closedTrades.reduce((s, t) => s + parseFloat(t.realized_pnl || '0'), 0);
  const wins = closedTrades.filter((t) => parseFloat(t.realized_pnl || '0') > 0).length;

  return (
    <div className="page trade-history-page animate-fade-in">
      <div className="page__header">
        <h1 className="page__title">Trade History</h1>
        <p className="page__subtitle">Complete log of all executed trades</p>
      </div>

      {/* Summary */}
      <div className="trade-history-page__summary card">
        <div className="trade-history-page__stat">
          <span className="label">Total Trades</span>
          <span className="trade-history-page__stat-value">{trades.length}</span>
        </div>
        <div className="trade-history-page__stat">
          <span className="label">Closed</span>
          <span className="trade-history-page__stat-value">{closedTrades.length}</span>
        </div>
        <div className="trade-history-page__stat">
          <span className="label">Wins</span>
          <span className="trade-history-page__stat-value text-profit">{wins}</span>
        </div>
        <div className="trade-history-page__stat">
          <span className="label">Losses</span>
          <span className="trade-history-page__stat-value text-loss">{closedTrades.length - wins}</span>
        </div>
        <div className="trade-history-page__stat">
          <span className="label">Total P&L</span>
          <PnlDisplay value={totalPnl} size="lg" suffix=" USD" />
        </div>
      </div>

      {/* Table */}
      {trades.length > 0 ? (
        <div className="table-container">
          <table>
            <thead>
              <tr>
                <th>Date</th>
                <th>Symbol</th>
                <th>Exchange</th>
                <th>Side</th>
                <th className="text-right">Entry</th>
                <th className="text-right">Exit</th>
                <th className="text-right">Qty</th>
                <th className="text-right">Realized P&L</th>
                <th>Status</th>
              </tr>
            </thead>
            <tbody>
              {trades.map((trade, i) => (
                <tr key={trade.id || i}>
                  <td className="text-muted" style={{ fontSize: 'var(--text-xs)' }}>
                    {new Date(trade.opened_at).toLocaleDateString()}
                  </td>
                  <td><strong>{trade.symbol}</strong></td>
                  <td><span className="badge badge-info">{trade.exchange}</span></td>
                  <td>
                    <StatusBadge
                      status={trade.side === 'buy' ? 'active' : 'error'}
                      label={trade.side.toUpperCase()}
                    />
                  </td>
                  <td className="text-right text-mono">{parseFloat(trade.entry_price).toLocaleString()}</td>
                  <td className="text-right text-mono">
                    {trade.exit_price ? parseFloat(trade.exit_price).toLocaleString() : '—'}
                  </td>
                  <td className="text-right text-mono">{trade.quantity}</td>
                  <td className="text-right">
                    {trade.realized_pnl ? (
                      <PnlDisplay value={trade.realized_pnl} />
                    ) : (
                      <span className="text-muted">—</span>
                    )}
                  </td>
                  <td>
                    <StatusBadge
                      status={trade.closed_at ? 'connected' : 'pending'}
                      label={trade.closed_at ? 'Closed' : 'Open'}
                    />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <div className="trade-history-page__empty card">
          <ScrollText size={48} strokeWidth={1} />
          <h3>No Trades Yet</h3>
          <p>Trade history will be recorded here when positions are opened and closed.</p>
        </div>
      )}
    </div>
  );
}

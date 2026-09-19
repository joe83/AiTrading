import { useEffect } from 'react';
import { Briefcase } from 'lucide-react';
import { PnlDisplay } from '../components/PnlDisplay';
import { StatusBadge } from '../components/StatusBadge';
import { useTradingStore } from '../stores/tradingStore';
import './PositionsPage.css';

export function PositionsPage() {
  const { positions, fetchPositions } = useTradingStore();

  useEffect(() => {
    fetchPositions();
    const interval = setInterval(fetchPositions, 5000);
    return () => clearInterval(interval);
  }, []);

  return (
    <div className="page positions-page animate-fade-in">
      <div className="page__header">
        <h1 className="page__title">Positions</h1>
        <p className="page__subtitle">Active trading positions across all exchanges</p>
      </div>

      {/* Summary Bar */}
      <div className="positions-page__summary card">
        <div className="positions-page__summary-item">
          <span className="label">Total Positions</span>
          <span className="positions-page__summary-value">{positions.length}</span>
        </div>
        <div className="positions-page__summary-item">
          <span className="label">Total P&L</span>
          <PnlDisplay
            value={positions.reduce((sum, p) => sum + parseFloat(p.unrealized_pnl || '0'), 0)}
            size="lg"
            suffix=" USD"
          />
        </div>
      </div>

      {/* Positions Table */}
      {positions.length > 0 ? (
        <div className="table-container">
          <table>
            <thead>
              <tr>
                <th>Symbol</th>
                <th>Exchange</th>
                <th>Side</th>
                <th className="text-right">Entry Price</th>
                <th className="text-right">Quantity</th>
                <th className="text-right">Unrealized P&L</th>
                <th className="text-right">Stop Loss</th>
                <th className="text-right">Take Profit</th>
                <th>Opened</th>
              </tr>
            </thead>
            <tbody>
              {positions.map((pos, i) => (
                <tr key={pos.id || i}>
                  <td><span className="positions-page__symbol">{pos.symbol}</span></td>
                  <td><span className="badge badge-info">{pos.exchange}</span></td>
                  <td>
                    <StatusBadge
                      status={pos.side === 'buy' ? 'active' : 'error'}
                      label={pos.side.toUpperCase()}
                    />
                  </td>
                  <td className="text-right text-mono">{parseFloat(pos.entry_price).toLocaleString()}</td>
                  <td className="text-right text-mono">{pos.quantity}</td>
                  <td className="text-right">
                    <PnlDisplay value={pos.unrealized_pnl} />
                  </td>
                  <td className="text-right text-mono text-loss">
                    {pos.stop_loss ? parseFloat(pos.stop_loss).toLocaleString() : '—'}
                  </td>
                  <td className="text-right text-mono text-profit">
                    {pos.take_profit ? parseFloat(pos.take_profit).toLocaleString() : '—'}
                  </td>
                  <td className="text-muted" style={{ fontSize: 'var(--text-xs)' }}>
                    {new Date(pos.opened_at).toLocaleString()}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <div className="positions-page__empty card">
          <Briefcase size={48} strokeWidth={1} />
          <h3>No Open Positions</h3>
          <p>Positions will appear here when trades are opened by the AI or manually.</p>
        </div>
      )}
    </div>
  );
}

import { useEffect } from 'react';
import { Outlet } from 'react-router-dom';
import { Sidebar } from './Sidebar';
import { LiveClock } from '../components/LiveClock';
import { StatusBadge } from '../components/StatusBadge';
import { ToastContainer } from '../components/Toast';
import { useTradingStore } from '../stores/tradingStore';
import './DashboardLayout.css';

export function DashboardLayout() {
  const {
    wsConnected,
    systemStatus,
    dashboard,
    prices,
    initWebSocket,
    disconnectWebSocket,
    fetchDashboard,
    fetchSystemStatus,
  } = useTradingStore();

  useEffect(() => {
    initWebSocket();
    fetchDashboard();
    fetchSystemStatus();

    const interval = setInterval(() => {
      fetchDashboard();
      fetchSystemStatus();
    }, 10000);

    return () => {
      disconnectWebSocket();
      clearInterval(interval);
    };
  }, []);

  const tradingMode = dashboard?.trading_mode?.toLowerCase().includes('auto') ? 'auto' : 'manual';
  const isPaused = dashboard?.is_paused ?? false;

  return (
    <div className="dashboard-layout">
      <Sidebar />

      <div className="dashboard-layout__main">
        {/* Top Bar */}
        <header className="topbar">
          <div className="topbar__left">
            <StatusBadge
              status={wsConnected ? 'connected' : 'disconnected'}
              pulse={wsConnected}
            />
            <StatusBadge status={isPaused ? 'paused' : tradingMode} />
          </div>
          <div className="topbar__center">
            <LiveClock />
          </div>
          <div className="topbar__right">
            {systemStatus && (
              <span className="topbar__uptime text-muted text-mono">
                Uptime: {formatUptime(systemStatus.uptime_secs)}
              </span>
            )}
          </div>
        </header>

        {/* Main Content */}
        <main className="dashboard-layout__content">
          <Outlet />
        </main>

        {/* Bottom Ticker */}
        <footer className="bottombar">
          <div className="bottombar__ticker">
            <div className="bottombar__ticker-track">
              {prices.size > 0 ? (
                Array.from(prices.values()).map((p, i) => (
                  <span key={`${p.symbol}-${i}`} className="bottombar__item">
                    <span className="bottombar__symbol">{p.symbol}</span>
                    <span className={`bottombar__price ${p.change_pct >= 0 ? 'text-profit' : 'text-loss'}`}>
                      {parseFloat(p.price).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 6 })}
                    </span>
                    <span className={`bottombar__change ${p.change_pct >= 0 ? 'text-profit' : 'text-loss'}`}>
                      {p.change_pct >= 0 ? '+' : ''}{p.change_pct.toFixed(2)}%
                    </span>
                  </span>
                ))
              ) : (
                <span className="bottombar__waiting">Waiting for price data...</span>
              )}
            </div>
          </div>
        </footer>
      </div>

      <ToastContainer />
    </div>
  );
}

function formatUptime(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return `${h}h ${m}m`;
}

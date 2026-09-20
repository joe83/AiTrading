import { useEffect } from 'react';
import { Link } from 'react-router-dom';
import { Settings, Pause, Play, Shield, Cpu, Key, ArrowRight } from 'lucide-react';
import { StatusBadge } from '../components/StatusBadge';
import { useTradingStore } from '../stores/tradingStore';
import './SettingsPage.css';

export function SettingsPage() {
  const {
    dashboard,
    systemStatus,
    fetchDashboard,
    fetchSystemStatus,
    setTradingMode,
    pauseTrading,
    resumeTrading,
  } = useTradingStore();

  useEffect(() => {
    fetchDashboard();
    fetchSystemStatus();
  }, []);

  const isAuto = dashboard?.trading_mode?.toLowerCase().includes('auto');
  const isPaused = dashboard?.is_paused ?? false;

  return (
    <div className="page settings-page animate-fade-in">
      <div className="page__header">
        <h1 className="page__title">Settings</h1>
        <p className="page__subtitle">Trading mode, risk parameters, and system controls</p>
      </div>

      {/* API Keys & Integrations Card */}
      <div className="card settings-page__section" style={{ borderColor: 'rgba(168, 85, 247, 0.3)' }}>
        <div className="settings-page__section-header">
          <Key size={20} style={{ color: '#c084fc' }} />
          <h2 className="card__title" style={{ marginBottom: 0 }}>API Keys & Provider Integrations</h2>
        </div>
        <p className="settings-page__desc">
          Configure API credentials and endpoints for <strong>xAI Grok AI</strong>, <strong>MEXC Crypto</strong>, <strong>Alpaca Equities</strong>, and <strong>IC Markets Forex</strong>. Changes apply instantly without server restarts.
        </p>
        <div>
          <Link to="/api-keys" className="btn btn-primary" style={{ display: 'inline-flex', alignItems: 'center', gap: '8px' }}>
            <Key size={16} /> Manage API Keys <ArrowRight size={16} />
          </Link>
        </div>
      </div>

      {/* Trading Mode */}
      <div className="card settings-page__section">
        <div className="settings-page__section-header">
          <Cpu size={20} />
          <h2 className="card__title" style={{ marginBottom: 0 }}>Trading Mode</h2>
        </div>
        <p className="settings-page__desc">
          In <strong>Auto</strong> mode, the AI executes trades automatically based on Grok analysis.
          In <strong>Manual</strong> mode, signals require your approval before execution.
        </p>
        <div className="settings-page__mode-toggle">
          <button
            className={`settings-page__mode-btn ${!isAuto ? 'settings-page__mode-btn--active' : ''}`}
            onClick={() => setTradingMode('manual')}
          >
            Manual
          </button>
          <button
            className={`settings-page__mode-btn ${isAuto ? 'settings-page__mode-btn--active settings-page__mode-btn--auto' : ''}`}
            onClick={() => setTradingMode('auto')}
          >
            Auto
          </button>
        </div>
        <div className="settings-page__current">
          Current: <StatusBadge status={isAuto ? 'auto' : 'manual'} />
        </div>
      </div>

      {/* Emergency Controls */}
      <div className="card settings-page__section">
        <div className="settings-page__section-header">
          <Shield size={20} />
          <h2 className="card__title" style={{ marginBottom: 0 }}>Emergency Controls</h2>
        </div>
        <p className="settings-page__desc">
          Immediately pause or resume all trading activity. Pausing stops new trade executions but keeps existing positions open.
        </p>
        <div className="settings-page__emergency">
          {isPaused ? (
            <button className="btn btn-success btn-lg" onClick={resumeTrading}>
              <Play size={20} /> Resume Trading
            </button>
          ) : (
            <button className="btn btn-danger btn-lg settings-page__pause-btn" onClick={pauseTrading}>
              <Pause size={20} /> Pause All Trading
            </button>
          )}
          <StatusBadge status={isPaused ? 'paused' : 'active'} pulse={!isPaused} />
        </div>
      </div>

      {/* Risk Parameters */}
      <div className="card settings-page__section">
        <div className="settings-page__section-header">
          <Shield size={20} />
          <h2 className="card__title" style={{ marginBottom: 0 }}>Risk Parameters</h2>
        </div>
        <p className="settings-page__desc">
          Risk management parameters configured in the server .env file. Restart the server to apply changes.
        </p>
        <div className="settings-page__params">
          <div className="settings-page__param">
            <span className="label">Max Position Size</span>
            <span className="settings-page__param-value">5.0%</span>
          </div>
          <div className="settings-page__param">
            <span className="label">Max Daily Loss</span>
            <span className="settings-page__param-value text-loss">3.0%</span>
          </div>
          <div className="settings-page__param">
            <span className="label">Max Concurrent Positions</span>
            <span className="settings-page__param-value">5</span>
          </div>
          <div className="settings-page__param">
            <span className="label">Default Stop Loss</span>
            <span className="settings-page__param-value text-loss">2.0%</span>
          </div>
          <div className="settings-page__param">
            <span className="label">Default Take Profit</span>
            <span className="settings-page__param-value text-profit">4.0%</span>
          </div>
          <div className="settings-page__param">
            <span className="label">Analysis Interval</span>
            <span className="settings-page__param-value">30s</span>
          </div>
        </div>
      </div>

      {/* System Info */}
      {systemStatus && (
        <div className="card settings-page__section">
          <div className="settings-page__section-header">
            <Settings size={20} />
            <h2 className="card__title" style={{ marginBottom: 0 }}>System Info</h2>
          </div>
          <div className="settings-page__params">
            <div className="settings-page__param">
              <span className="label">Server Status</span>
              <StatusBadge status="connected" label={systemStatus.server} pulse />
            </div>
            <div className="settings-page__param">
              <span className="label">Uptime</span>
              <span className="settings-page__param-value text-mono">
                {Math.floor(systemStatus.uptime_secs / 3600)}h {Math.floor((systemStatus.uptime_secs % 3600) / 60)}m
              </span>
            </div>
            <div className="settings-page__param">
              <span className="label">Grok Tokens Used</span>
              <span className="settings-page__param-value text-mono">
                {systemStatus.grok_tokens_used.toLocaleString()}
              </span>
            </div>
            <div className="settings-page__param">
              <span className="label">Open Positions</span>
              <span className="settings-page__param-value">{systemStatus.open_positions}</span>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

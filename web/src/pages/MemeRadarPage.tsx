import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  Flame,
  Sparkles,
  TrendingUp,
  TrendingDown,
  RefreshCw,
  ExternalLink,
  ShieldAlert,
  Loader2,
  LineChart,
  Layers,
  Activity,
  Radio,
} from 'lucide-react';
import {
  apiClient,
  type MemeRadarReport,
} from '../api/client';
import './MemeRadarPage.css';

export function MemeRadarPage() {
  const navigate = useNavigate();
  const [report, setReport] = useState<MemeRadarReport | null>(null);
  const [loading, setLoading] = useState<boolean>(true);
  const [scanning, setScanning] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);
  const [filterTradeableOnly, setFilterTradeableOnly] = useState<boolean>(false);
  const [sortBy, setSortBy] = useState<'velocity' | 'change' | 'volume' | 'sentiment'>('velocity');

  const fetchRadar = async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await apiClient.getMemeRadar();
      if (data.report) {
        setReport(data.report);
      }
    } catch (err: unknown) {
      const e = err as Error;
      setError(e.message || 'Failed to load meme radar data');
    } finally {
      setLoading(false);
    }
  };

  const triggerScan = async () => {
    setScanning(true);
    setError(null);
    try {
      const res = await apiClient.scanMemeRadar();
      if (res.report) {
        setReport(res.report);
      }
    } catch (err: unknown) {
      const e = err as Error;
      setError(e.message || 'Live radar scan failed');
    } finally {
      setScanning(false);
    }
  };

  useEffect(() => {
    fetchRadar();
    // Auto-refresh every 60 seconds
    const interval = setInterval(fetchRadar, 60000);
    return () => clearInterval(interval);
  }, []);

  // Filter and sort tokens
  const displayedTokens = (report?.tokens || [])
    .filter((token) => (filterTradeableOnly ? token.is_tradeable_on_mexc : true))
    .sort((a, b) => {
      if (sortBy === 'velocity') {
        return b.viral_velocity - a.viral_velocity;
      }
      if (sortBy === 'change') {
        return (b.price_change_24h_pct || 0) - (a.price_change_24h_pct || 0);
      }
      if (sortBy === 'volume') {
        return (b.volume_24h_usdt || 0) - (a.volume_24h_usdt || 0);
      }
      if (sortBy === 'sentiment') {
        return b.sentiment_score - a.sentiment_score;
      }
      return 0;
    });

  const avgVelocity = report?.tokens?.length
    ? Math.round(
        report.tokens.reduce((acc, t) => acc + t.viral_velocity, 0) /
          report.tokens.length
      )
    : 0;

  const mexcAvailableCount = report?.tokens?.filter((t) => t.is_tradeable_on_mexc).length || 0;

  const formatPrice = (val?: number | null) => {
    if (val === undefined || val === null) return '—';
    if (val < 0.0001) return `$${val.toFixed(8)}`;
    if (val < 1) return `$${val.toFixed(5)}`;
    return `$${val.toFixed(2)}`;
  };

  const formatVolume = (val?: number | null) => {
    if (val === undefined || val === null) return '—';
    if (val >= 1_000_000) return `$${(val / 1_000_000).toFixed(2)}M`;
    if (val >= 1_000) return `$${(val / 1_000).toFixed(1)}K`;
    return `$${val.toFixed(0)}`;
  };

  const getRiskBadgeClass = (risk: string) => {
    switch (risk?.toLowerCase()) {
      case 'low':
        return 'badge-info';
      case 'medium':
        return 'badge-warning';
      case 'extreme':
        return 'badge-danger';
      default:
        return 'badge-warning';
    }
  };

  return (
    <div className="page meme-radar-page animate-fade-in">
      {/* Header */}
      <div className="page__header meme-radar__header">
        <div>
          <div className="meme-radar__title-wrap">
            <div className="meme-radar__fire-icon">
              <Flame size={24} />
            </div>
            <h1 className="page__title">Meme & Social Sentiment Radar</h1>
          </div>
          <p className="page__subtitle">
            Real-time Crypto Twitter attention surges tracked by <strong>xAI Grok</strong>, matched with live <strong>MEXC Spot</strong> market liquidity.
          </p>
        </div>

        <div className="meme-radar__controls">
          <button
            className="btn btn-primary meme-radar__scan-btn"
            onClick={triggerScan}
            disabled={scanning || loading}
          >
            {scanning ? (
              <>
                <Loader2 size={16} className="spinner" /> Scanning CT Firehose...
              </>
            ) : (
              <>
                <Radio size={16} className="radar-pulse-icon" /> Scan Crypto Twitter Now
              </>
            )}
          </button>
          <button
            className="btn btn-secondary btn-sm"
            onClick={fetchRadar}
            disabled={loading}
            title="Refresh current data"
          >
            <RefreshCw size={14} className={loading ? 'spinner' : ''} />
          </button>
        </div>
      </div>

      {error && (
        <div className="alert alert-error animate-slide-down">
          <ShieldAlert size={18} />
          <span>{error}</span>
        </div>
      )}

      {/* Metrics Row */}
      <div className="meme-radar__stats-grid">
        <div className="card meme-stat-card">
          <div className="meme-stat-card__icon meme-stat-card__icon--purple">
            <Sparkles size={20} />
          </div>
          <div className="meme-stat-card__content">
            <span className="meme-stat-card__label">Current X Narrative</span>
            <span className="meme-stat-card__value meme-stat-card__value--text">
              {report?.top_narrative_theme || 'Analyzing CT...'}
            </span>
          </div>
        </div>

        <div className="card meme-stat-card">
          <div className="meme-stat-card__icon meme-stat-card__icon--orange">
            <Flame size={20} />
          </div>
          <div className="meme-stat-card__content">
            <span className="meme-stat-card__label">Avg Viral Velocity</span>
            <span className="meme-stat-card__value">
              {avgVelocity} <span className="meme-stat-card__unit">/ 100</span>
            </span>
          </div>
        </div>

        <div className="card meme-stat-card">
          <div className="meme-stat-card__icon meme-stat-card__icon--cyan">
            <Layers size={20} />
          </div>
          <div className="meme-stat-card__content">
            <span className="meme-stat-card__label">Active Viral Tokens</span>
            <span className="meme-stat-card__value">
              {report?.tokens?.length || 0}{' '}
              <span className="meme-stat-card__unit">({mexcAvailableCount} on MEXC)</span>
            </span>
          </div>
        </div>

        <div className="card meme-stat-card">
          <div className="meme-stat-card__icon meme-stat-card__icon--green">
            <Activity size={20} />
          </div>
          <div className="meme-stat-card__content">
            <span className="meme-stat-card__label">Intelligence Feed</span>
            <span className="meme-stat-card__value meme-stat-card__value--text" style={{ fontSize: '0.875rem' }}>
              {report?.source || 'xAI Grok CT Stream'}
            </span>
          </div>
        </div>
      </div>

      {/* Narrative Synthesis Banner */}
      {report?.narrative_summary && (
        <div className="card meme-radar__narrative-card animate-slide-down">
          <div className="meme-radar__narrative-header">
            <div className="meme-radar__grok-badge">
              <Sparkles size={14} /> Grok CT Narrative Intelligence
            </div>
            {report?.scanned_at && (
              <span className="meme-radar__timestamp">
                Scanned {new Date(report.scanned_at).toLocaleTimeString()} UTC
              </span>
            )}
          </div>
          <p className="meme-radar__narrative-body">{report.narrative_summary}</p>
        </div>
      )}

      {/* Filter & Sort Bar */}
      <div className="meme-radar__filters-bar">
        <div className="meme-radar__filter-left">
          <label className="checkbox-label">
            <input
              type="checkbox"
              checked={filterTradeableOnly}
              onChange={(e) => setFilterTradeableOnly(e.target.checked)}
            />
            <span>MEXC Spot Available Only ({mexcAvailableCount})</span>
          </label>
        </div>

        <div className="meme-radar__sort-controls">
          <span className="meme-radar__sort-label">Sort By:</span>
          <button
            className={`btn-filter ${sortBy === 'velocity' ? 'btn-filter--active' : ''}`}
            onClick={() => setSortBy('velocity')}
          >
            🔥 Viral Velocity
          </button>
          <button
            className={`btn-filter ${sortBy === 'change' ? 'btn-filter--active' : ''}`}
            onClick={() => setSortBy('change')}
          >
            📈 24h Change
          </button>
          <button
            className={`btn-filter ${sortBy === 'volume' ? 'btn-filter--active' : ''}`}
            onClick={() => setSortBy('volume')}
          >
            💵 Volume
          </button>
          <button
            className={`btn-filter ${sortBy === 'sentiment' ? 'btn-filter--active' : ''}`}
            onClick={() => setSortBy('sentiment')}
          >
            🧠 Social Sentiment
          </button>
        </div>
      </div>

      {/* Token Radar Grid */}
      {loading && !report ? (
        <div className="meme-radar__loading">
          <Loader2 size={36} className="spinner" />
          <span>Intercepting Crypto Twitter meme trends & validating MEXC pairs...</span>
        </div>
      ) : (
        <div className="meme-radar__tokens-grid">
          {displayedTokens.map((token) => {
            const isPositive = (token.price_change_24h_pct || 0) >= 0;
            const velocityPct = Math.min(Math.max(token.viral_velocity, 0), 100);

            return (
              <div key={token.symbol} className="card meme-token-card">
                {/* Card Top */}
                <div className="meme-token-card__header">
                  <div className="meme-token-card__identity">
                    <div className="meme-token-card__symbol-box">
                      <span className="meme-token-card__symbol">{token.symbol}</span>
                      <span className="meme-token-card__cashtag">{token.cashtag}</span>
                    </div>
                    <div>
                      <h3 className="meme-token-card__name">{token.name}</h3>
                      <span className="badge badge-info meme-token-card__narrative-tag">
                        {token.narrative}
                      </span>
                    </div>
                  </div>

                  <div className="meme-token-card__badges">
                    <span className={`badge ${getRiskBadgeClass(token.risk_level)}`}>
                      {token.risk_level} Risk
                    </span>
                    {token.is_tradeable_on_mexc && (
                      <span className="badge badge-success">MEXC Live</span>
                    )}
                  </div>
                </div>

                {/* Viral Velocity Meter */}
                <div className="meme-token-card__velocity-section">
                  <div className="meme-token-card__velocity-header">
                    <span className="meme-token-card__section-label">
                      <Flame size={14} className="fire-color" /> Viral Momentum Index
                    </span>
                    <span className="meme-token-card__velocity-score">
                      {token.viral_velocity} <span>/ 100</span>
                    </span>
                  </div>
                  <div className="meme-velocity-bar">
                    <div
                      className="meme-velocity-bar__fill"
                      style={{ width: `${velocityPct}%` }}
                    />
                  </div>
                </div>

                {/* Sentiment & Catalysts */}
                <div className="meme-token-card__sentiment-box">
                  <div className="meme-token-card__sentiment-row">
                    <span className="meme-token-card__section-label">Social Sentiment</span>
                    <span
                      className={`meme-token-card__sentiment-label ${
                        token.sentiment_score >= 0.5
                          ? 'sentiment--hyper'
                          : token.sentiment_score > 0
                          ? 'sentiment--bullish'
                          : 'sentiment--bearish'
                      }`}
                    >
                      {token.sentiment_label} ({(token.sentiment_score * 100).toFixed(0)}%)
                    </span>
                  </div>

                  {token.catalysts && token.catalysts.length > 0 && (
                    <div className="meme-token-card__catalysts">
                      {token.catalysts.map((cat, i) => (
                        <span key={i} className="meme-token-card__catalyst-pill">
                          • {cat}
                        </span>
                      ))}
                    </div>
                  )}
                </div>

                {/* MEXC Live Market Data */}
                <div className="meme-token-card__market-box">
                  <div className="meme-token-card__market-item">
                    <span className="market-item__label">MEXC Price</span>
                    <span className="market-item__value">
                      {formatPrice(token.current_price_usdt)}
                    </span>
                  </div>

                  <div className="meme-token-card__market-item">
                    <span className="market-item__label">24h Change</span>
                    <span
                      className={`market-item__value ${
                        isPositive ? 'text-success' : 'text-danger'
                      }`}
                    >
                      {isPositive ? (
                        <TrendingUp size={13} style={{ display: 'inline', marginRight: 2 }} />
                      ) : (
                        <TrendingDown size={13} style={{ display: 'inline', marginRight: 2 }} />
                      )}
                      {token.price_change_24h_pct !== null && token.price_change_24h_pct !== undefined
                        ? `${token.price_change_24h_pct >= 0 ? '+' : ''}${token.price_change_24h_pct.toFixed(2)}%`
                        : '—'}
                    </span>
                  </div>

                  <div className="meme-token-card__market-item">
                    <span className="market-item__label">24h Volume</span>
                    <span className="market-item__value">
                      {formatVolume(token.volume_24h_usdt)}
                    </span>
                  </div>
                </div>

                {/* Actions */}
                <div className="meme-token-card__actions">
                  <button
                    className="btn btn-secondary btn-sm"
                    onClick={() => navigate(`/charts?symbol=${token.mexc_symbol}`)}
                  >
                    <LineChart size={14} /> View Chart
                  </button>

                  {token.is_tradeable_on_mexc && (
                    <a
                      href={`https://www.mexc.com/exchange/${token.symbol}_USDT`}
                      target="_blank"
                      rel="noreferrer"
                      className="btn btn-primary btn-sm meme-token-card__trade-btn"
                    >
                      Trade on MEXC <ExternalLink size={12} />
                    </a>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

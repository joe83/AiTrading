import { useEffect, useState } from 'react';
import { Globe, Wifi, WifiOff, Clock, Bot, Zap, ShieldOff, Loader2 } from 'lucide-react';
import { StatusBadge } from '../components/StatusBadge';
import { useTradingStore } from '../stores/tradingStore';
import './ExchangesPage.css';

const EXCHANGE_INFO: Record<string, { name: string; type: string; markets: string; icon: string }> = {
  MEXC:         { name: 'MEXC',        type: 'Crypto',  markets: 'BTC, ETH, SOL, 1800+ pairs', icon: '₿' },
  Mexc:         { name: 'MEXC',        type: 'Crypto',  markets: 'BTC, ETH, SOL, 1800+ pairs', icon: '₿' },
  mexc:         { name: 'MEXC',        type: 'Crypto',  markets: 'BTC, ETH, SOL, 1800+ pairs', icon: '₿' },
  Alpaca:       { name: 'Alpaca',      type: 'US Stocks',markets: 'AAPL, TSLA, GOOGL, 8000+ stocks', icon: '📈' },
  alpaca:       { name: 'Alpaca',      type: 'US Stocks',markets: 'AAPL, TSLA, GOOGL, 8000+ stocks', icon: '📈' },
  'IC Markets': { name: 'IC Markets',  type: 'Forex',   markets: 'EUR/USD, GBP/USD, 60+ pairs', icon: '💱' },
  'ic_markets': { name: 'IC Markets',  type: 'Forex',   markets: 'EUR/USD, GBP/USD, 60+ pairs', icon: '💱' },
  IcMarkets:    { name: 'IC Markets',  type: 'Forex',   markets: 'EUR/USD, GBP/USD, 60+ pairs', icon: '💱' },
  Binance:      { name: 'Binance',     type: 'Crypto',  markets: 'BTC, ETH, BNB, 1400+ pairs', icon: '🔶' },
  binance:      { name: 'Binance',     type: 'Crypto',  markets: 'BTC, ETH, BNB, 1400+ pairs', icon: '🔶' },
  Bybit:        { name: 'Bybit',       type: 'Crypto',  markets: 'BTC, ETH, SOL, 800+ pairs', icon: '🟡' },
  bybit:        { name: 'Bybit',       type: 'Crypto',  markets: 'BTC, ETH, SOL, 800+ pairs', icon: '🟡' },
};

export function ExchangesPage() {
  const { exchanges, fetchExchanges, toggleExchange, addToast } = useTradingStore();
  const [toggling, setToggling] = useState<Record<string, boolean>>({});

  useEffect(() => {
    fetchExchanges();
    const interval = setInterval(fetchExchanges, 10000);
    return () => clearInterval(interval);
  }, []);

  const handleToggle = async (rawKey: string, currentEnabled: boolean, isConnected: boolean) => {
    if (!isConnected) {
      addToast({
        type: 'warning',
        title: 'API Keys Required',
        message: `Please add and verify valid API keys in API Keys settings before enabling auto-bot trading.`,
      });
      return;
    }

    setToggling((prev) => ({ ...prev, [rawKey]: true }));
    try {
      await toggleExchange(rawKey, !currentEnabled);
    } finally {
      setToggling((prev) => ({ ...prev, [rawKey]: false }));
    }
  };

  return (
    <div className="page exchanges-page animate-fade-in">
      <div className="page__header">
        <h1 className="page__title">Exchanges</h1>
        <p className="page__subtitle">
          Manage exchange connections and configure automated bot trading permissions
        </p>
      </div>

      <div className="exchanges-page__grid">
        {exchanges.length > 0 ? (
          exchanges.map((ex, i) => {
            const rawKey = ex.exchange || (ex as { id?: string }).id || '';
            const info =
              EXCHANGE_INFO[rawKey] ||
              EXCHANGE_INFO[rawKey.toLowerCase()] ||
              { name: rawKey || 'Exchange', type: 'Crypto', markets: '—', icon: '🌐' };
            const isToggling = !!toggling[rawKey];
            const isAutoActive = ex.connected && !!ex.enabled;

            return (
              <div
                key={i}
                className={`exchanges-page__card card ${
                  ex.connected
                    ? isAutoActive
                      ? 'exchanges-page__card--auto-active'
                      : 'exchanges-page__card--connected'
                    : 'exchanges-page__card--disconnected'
                }`}
              >
                <div className="exchanges-page__card-header">
                  <span className="exchanges-page__icon">{info.icon}</span>
                  <div>
                    <h3 className="exchanges-page__name">{info.name}</h3>
                    <div className="exchanges-page__badges">
                      <span className="badge badge-info">{info.type}</span>
                      {ex.connected ? (
                        ex.enabled ? (
                          <span className="badge badge-success exchanges-page__tag-badge">
                            <Zap size={10} /> Auto-Bot ON
                          </span>
                        ) : (
                          <span className="badge badge-warning exchanges-page__tag-badge">
                            <ShieldOff size={10} /> Auto-Bot OFF
                          </span>
                        )
                      ) : null}
                    </div>
                  </div>
                  <StatusBadge
                    status={ex.connected ? 'connected' : 'disconnected'}
                    pulse={ex.connected}
                  />
                </div>

                <div className="exchanges-page__details">
                  <div className="exchanges-page__detail">
                    {ex.connected ? <Wifi size={14} className="text-profit" /> : <WifiOff size={14} />}
                    <span>{ex.connected ? 'API Connected & Authenticated' : 'Not Connected'}</span>
                  </div>
                  <div className="exchanges-page__detail">
                    <Globe size={14} />
                    <span>{info.markets}</span>
                  </div>
                  <div className="exchanges-page__detail">
                    <Clock size={14} />
                    <span>
                      {info.type === 'Crypto'
                        ? '24/7 Trading'
                        : info.type === 'US Stocks'
                        ? 'Mon-Fri 9:30-16:00 EST'
                        : 'Sun-Fri 24h'}
                    </span>
                  </div>
                </div>

                {/* Auto-Trading Bot Permission Control */}
                <div className="exchanges-page__bot-row">
                  <div className="exchanges-page__bot-info">
                    <div className="exchanges-page__bot-header">
                      <Bot
                        size={16}
                        className={isAutoActive ? 'text-profit' : 'text-secondary'}
                      />
                      <span className="exchanges-page__bot-title">Auto Bot Transactions</span>
                    </div>
                    <span className="exchanges-page__bot-subtitle">
                      {ex.connected
                        ? ex.enabled
                          ? 'Bot is allowed to execute automated trades'
                          : 'Bot execution disabled — manual only'
                        : 'Requires connected API keys to enable'}
                    </span>
                  </div>

                  <button
                    type="button"
                    disabled={isToggling}
                    onClick={() => handleToggle(rawKey, !!ex.enabled, ex.connected)}
                    className={`exchange-toggle-btn ${
                      !ex.connected
                        ? 'exchange-toggle-btn--unconnected'
                        : ex.enabled
                        ? 'exchange-toggle-btn--active'
                        : 'exchange-toggle-btn--inactive'
                    }`}
                    title={
                      !ex.connected
                        ? 'Add API keys first to enable auto trading'
                        : ex.enabled
                        ? 'Click to disable automated bot trades on this exchange'
                        : 'Click to enable automated bot trades on this exchange'
                    }
                  >
                    {isToggling ? (
                      <Loader2 size={13} className="animate-spin" />
                    ) : isAutoActive ? (
                      <>
                        <span className="exchange-toggle-dot exchange-toggle-dot--active" />
                        <span>ENABLED</span>
                      </>
                    ) : (
                      <>
                        <span className="exchange-toggle-dot" />
                        <span>DISABLED</span>
                      </>
                    )}
                  </button>
                </div>
              </div>
            );
          })
        ) : (
          <>
            {/* Show default cards when no exchange data */}
            {['MEXC', 'Alpaca', 'IC Markets'].map((name) => {
              const info = EXCHANGE_INFO[name]!;
              return (
                <div key={name} className="exchanges-page__card card exchanges-page__card--disconnected">
                  <div className="exchanges-page__card-header">
                    <span className="exchanges-page__icon">{info.icon}</span>
                    <div>
                      <h3 className="exchanges-page__name">{info.name}</h3>
                      <span className="badge badge-info">{info.type}</span>
                    </div>
                    <StatusBadge status="disconnected" />
                  </div>
                  <div className="exchanges-page__details">
                    <div className="exchanges-page__detail">
                      <WifiOff size={14} />
                      <span>Not Connected — Configure API keys in .env</span>
                    </div>
                    <div className="exchanges-page__detail">
                      <Globe size={14} />
                      <span>{info.markets}</span>
                    </div>
                  </div>
                </div>
              );
            })}
          </>
        )}
      </div>
    </div>
  );
}

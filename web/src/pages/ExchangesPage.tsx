import { useEffect } from 'react';
import { Globe, Wifi, WifiOff, Clock } from 'lucide-react';
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
};

export function ExchangesPage() {
  const { exchanges, fetchExchanges } = useTradingStore();

  useEffect(() => {
    fetchExchanges();
    const interval = setInterval(fetchExchanges, 10000);
    return () => clearInterval(interval);
  }, []);

  return (
    <div className="page exchanges-page animate-fade-in">
      <div className="page__header">
        <h1 className="page__title">Exchanges</h1>
        <p className="page__subtitle">Exchange connection status and details</p>
      </div>

      <div className="exchanges-page__grid">
        {exchanges.length > 0 ? (
          exchanges.map((ex, i) => {
            const info = EXCHANGE_INFO[ex.exchange] || { name: ex.exchange, type: 'Unknown', markets: '—', icon: '🌐' };
            return (
              <div key={i} className={`exchanges-page__card card ${ex.connected ? 'exchanges-page__card--connected' : 'exchanges-page__card--disconnected'}`}>
                <div className="exchanges-page__card-header">
                  <span className="exchanges-page__icon">{info.icon}</span>
                  <div>
                    <h3 className="exchanges-page__name">{info.name}</h3>
                    <span className="badge badge-info">{info.type}</span>
                  </div>
                  <StatusBadge
                    status={ex.connected ? 'connected' : 'disconnected'}
                    pulse={ex.connected}
                  />
                </div>

                <div className="exchanges-page__details">
                  <div className="exchanges-page__detail">
                    {ex.connected ? <Wifi size={14} /> : <WifiOff size={14} />}
                    <span>{ex.connected ? 'API Connected' : 'Not Connected'}</span>
                  </div>
                  <div className="exchanges-page__detail">
                    <Globe size={14} />
                    <span>{info.markets}</span>
                  </div>
                  <div className="exchanges-page__detail">
                    <Clock size={14} />
                    <span>{info.type === 'Crypto' ? '24/7 Trading' : info.type === 'US Stocks' ? 'Mon-Fri 9:30-16:00 EST' : 'Sun-Fri 24h'}</span>
                  </div>
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

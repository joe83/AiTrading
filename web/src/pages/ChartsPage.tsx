import { useEffect, useRef, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { createChart, CandlestickSeries, HistogramSeries, type IChartApi, type ISeriesApi, ColorType } from 'lightweight-charts';
import { api } from '../api/client';
import './ChartsPage.css';

const TIMEFRAMES = ['1m', '5m', '15m', '30m', '1h', '4h', '1d', '1w'];
const DEFAULT_SYMBOLS = ['BTCUSDT', 'ETHUSDT', 'SOLUSDT', 'BNBUSDT'];

function timeframeToSeconds(tf: string): number {
  switch (tf.toLowerCase()) {
    case '1m': return 60;
    case '5m': return 300;
    case '15m': return 900;
    case '30m': return 1800;
    case '1h':
    case '60m': return 3600;
    case '4h': return 14400;
    case '1d': return 86400;
    case '1w': return 604800;
    default: return 3600;
  }
}

function getPrecision(price: number): { precision: number; minMove: number } {
  if (price <= 0) return { precision: 2, minMove: 0.01 };
  if (price < 0.0001) return { precision: 8, minMove: 0.00000001 };
  if (price < 0.01) return { precision: 6, minMove: 0.000001 };
  if (price < 1) return { precision: 4, minMove: 0.0001 };
  return { precision: 2, minMove: 0.01 };
}

// Generate fallback demo OHLCV data for chart display if offline or exchange error
function generateDemoCandles(count: number, tf: string = '1h', symbol: string = 'BTCUSDT') {
  const step = timeframeToSeconds(tf);
  const candles = [];
  const now = Math.floor(Date.now() / 1000);
  const alignedNow = Math.floor(now / step) * step;

  let basePrice = 68000;
  const upper = symbol.toUpperCase();
  if (upper.includes('ETH')) basePrice = 3450;
  else if (upper.includes('SOL')) basePrice = 175;
  else if (upper.includes('BNB')) basePrice = 590;
  else if (upper.includes('PEPE')) basePrice = 0.00000437;
  else if (upper.includes('DOGE')) basePrice = 0.165;
  else if (upper.includes('SHIB')) basePrice = 0.000017;

  const { precision } = getPrecision(basePrice);
  let price = basePrice * (0.92 + Math.random() * 0.05);

  for (let i = count; i >= 0; i--) {
    const open = price;
    const volPct = tf === '1m' ? 0.002 : tf === '5m' ? 0.004 : tf === '15m' ? 0.008 : tf === '30m' ? 0.012 : 0.02;
    const delta = (Math.random() - 0.49) * (open * volPct);
    const close = Math.max(open + delta, basePrice * 0.05);
    const high = Math.max(open, close) + Math.random() * (open * volPct * 0.6);
    const low = Math.min(open, close) - Math.random() * (open * volPct * 0.6);

    candles.push({
      time: (alignedNow - i * step) as number,
      open: parseFloat(open.toFixed(precision)),
      high: parseFloat(high.toFixed(precision)),
      low: parseFloat(low.toFixed(precision)),
      close: parseFloat(close.toFixed(precision)),
    });

    price = close;
  }
  return candles;
}

function generateDemoVolume(candles: { time: number; open: number; close: number }[]) {
  return candles.map((c) => ({
    time: c.time,
    value: Math.floor(Math.random() * 5000 + 500),
    color: c.close >= c.open ? 'rgba(0, 230, 118, 0.35)' : 'rgba(255, 23, 68, 0.35)',
  }));
}

export function ChartsPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const urlSymbol = searchParams.get('symbol')?.toUpperCase() || '';

  const chartContainerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | null>(null);
  const candleSeriesRef = useRef<ISeriesApi<'Candlestick'> | null>(null);
  const volumeSeriesRef = useRef<ISeriesApi<'Histogram'> | null>(null);

  const [symbols, setSymbols] = useState<string[]>(() => {
    const list = [...DEFAULT_SYMBOLS];
    if (urlSymbol && !list.includes(urlSymbol)) {
      list.unshift(urlSymbol);
    }
    return list;
  });

  const [activeSymbol, setActiveSymbol] = useState(urlSymbol || 'BTCUSDT');
  const [activeTimeframe, setActiveTimeframe] = useState('30m');
  const [dataSource, setDataSource] = useState<'live' | 'demo'>('live');
  const [isLoading, setIsLoading] = useState(false);
  const [lastPrice, setLastPrice] = useState<number | null>(null);

  // Sync activeSymbol if URL changes
  useEffect(() => {
    if (urlSymbol && urlSymbol !== activeSymbol) {
      setActiveSymbol(urlSymbol);
      if (!symbols.includes(urlSymbol)) {
        setSymbols((prev) => [urlSymbol, ...prev]);
      }
    }
  }, [urlSymbol]);

  const handleSelectSymbol = (sym: string) => {
    setActiveSymbol(sym);
    setSearchParams({ symbol: sym });
  };

  // Initialize Lightweight Charts instance
  useEffect(() => {
    if (!chartContainerRef.current) return;

    const chart = createChart(chartContainerRef.current, {
      layout: {
        background: { type: ColorType.Solid, color: '#0a0e17' },
        textColor: '#94a3b8',
        fontFamily: "'Inter', sans-serif",
        fontSize: 12,
      },
      grid: {
        vertLines: { color: 'rgba(255, 255, 255, 0.04)' },
        horzLines: { color: 'rgba(255, 255, 255, 0.04)' },
      },
      crosshair: {
        mode: 0,
        vertLine: { color: 'rgba(0, 212, 255, 0.3)', width: 1, style: 2, labelBackgroundColor: '#1a2332' },
        horzLine: { color: 'rgba(0, 212, 255, 0.3)', width: 1, style: 2, labelBackgroundColor: '#1a2332' },
      },
      rightPriceScale: {
        borderColor: 'rgba(255, 255, 255, 0.06)',
        scaleMargins: { top: 0.1, bottom: 0.2 },
      },
      timeScale: {
        borderColor: 'rgba(255, 255, 255, 0.06)',
        timeVisible: true,
        secondsVisible: false,
      },
      handleScale: { axisPressedMouseMove: { time: true, price: true } },
      handleScroll: { vertTouchDrag: true },
    });

    chartRef.current = chart;

    const candleSeries = chart.addSeries(CandlestickSeries, {
      upColor: '#00e676',
      downColor: '#ff1744',
      borderUpColor: '#00e676',
      borderDownColor: '#ff1744',
      wickUpColor: '#00e676',
      wickDownColor: '#ff1744',
    });
    candleSeriesRef.current = candleSeries;

    const volumeSeries = chart.addSeries(HistogramSeries, {
      priceFormat: { type: 'volume' },
      priceScaleId: '',
    });
    volumeSeries.priceScale().applyOptions({
      scaleMargins: { top: 0.85, bottom: 0 },
    });
    volumeSeriesRef.current = volumeSeries;

    chart.timeScale().fitContent();

    // Responsive resize
    const resizeObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const { width, height } = entry.contentRect;
        chart.applyOptions({ width, height });
      }
    });
    resizeObserver.observe(chartContainerRef.current);

    return () => {
      resizeObserver.disconnect();
      chart.remove();
    };
  }, []);

  // Update chart candles whenever activeSymbol or activeTimeframe changes
  useEffect(() => {
    if (!candleSeriesRef.current || !volumeSeriesRef.current) return;
    let isMounted = true;
    setIsLoading(true);

    api.getMarketCandles(activeSymbol, activeTimeframe, 250)
      .then((realCandles) => {
        if (!isMounted) return;
        if (realCandles && realCandles.length > 0) {
          const lastCandle = realCandles[realCandles.length - 1];
          setLastPrice(lastCandle.close);
          const { precision, minMove } = getPrecision(lastCandle.close);

          candleSeriesRef.current?.applyOptions({
            priceFormat: { type: 'price', precision, minMove },
          });

          candleSeriesRef.current?.setData(realCandles as any);
          volumeSeriesRef.current?.setData(
            realCandles.map((c) => ({
              time: c.time,
              value: c.volume,
              color: c.close >= c.open ? 'rgba(0, 230, 118, 0.35)' : 'rgba(255, 23, 68, 0.35)',
            })) as any,
          );
          chartRef.current?.timeScale().fitContent();
          setDataSource('live');
        } else {
          loadFallbackDemo();
        }
      })
      .catch(() => {
        if (!isMounted) return;
        loadFallbackDemo();
      })
      .finally(() => {
        if (isMounted) setIsLoading(false);
      });

    function loadFallbackDemo() {
      const demo = generateDemoCandles(200, activeTimeframe, activeSymbol);
      const lastCandle = demo[demo.length - 1];
      setLastPrice(lastCandle.close);
      const { precision, minMove } = getPrecision(lastCandle.close);

      candleSeriesRef.current?.applyOptions({
        priceFormat: { type: 'price', precision, minMove },
      });
      candleSeriesRef.current?.setData(demo as any);
      volumeSeriesRef.current?.setData(generateDemoVolume(demo) as any);
      chartRef.current?.timeScale().fitContent();
      setDataSource('demo');
    }

    return () => {
      isMounted = false;
    };
  }, [activeSymbol, activeTimeframe]);

  return (
    <div className="page charts-page animate-fade-in">
      <div className="charts-page__controls">
        {/* Symbol Selector */}
        <div className="charts-page__symbols">
          {symbols.map((sym) => (
            <button
              key={sym}
              className={`charts-page__symbol-btn ${activeSymbol === sym ? 'charts-page__symbol-btn--active' : ''}`}
              onClick={() => handleSelectSymbol(sym)}
            >
              {sym}
            </button>
          ))}
        </div>

        {/* Timeframe Selector */}
        <div className="charts-page__timeframes">
          {TIMEFRAMES.map((tf) => (
            <button
              key={tf}
              className={`charts-page__tf-btn ${activeTimeframe === tf ? 'charts-page__tf-btn--active' : ''}`}
              onClick={() => setActiveTimeframe(tf)}
            >
              {tf}
            </button>
          ))}
        </div>
      </div>

      {/* Chart */}
      <div className="charts-page__chart-wrapper">
        <div ref={chartContainerRef} className="charts-page__chart" />
      </div>

      {/* Chart Info Bar */}
      <div className="charts-page__info">
        <span className="charts-page__info-item">
          <span className="label">Symbol</span>
          <span className="text-mono font-bold text-accent">{activeSymbol}</span>
        </span>
        <span className="charts-page__info-item">
          <span className="label">Timeframe</span>
          <span className="text-mono">{activeTimeframe}</span>
        </span>
        <span className="charts-page__info-item">
          <span className="label">Exchange</span>
          <span className="text-mono">MEXC</span>
        </span>
        {lastPrice !== null && (
          <span className="charts-page__info-item">
            <span className="label">Last Price</span>
            <span className="text-mono font-bold">
              ${lastPrice < 0.001 ? lastPrice.toFixed(8) : lastPrice < 1 ? lastPrice.toFixed(4) : lastPrice.toFixed(2)}
            </span>
          </span>
        )}
        <span className="charts-page__info-item" style={{ marginLeft: 'auto' }}>
          <span className="label">Status</span>
          <span className={`charts-page__badge ${dataSource === 'live' ? 'charts-page__badge--live' : 'charts-page__badge--demo'}`}>
            <span className="charts-page__pulse" />
            {isLoading ? 'Updating...' : dataSource === 'live' ? 'Live MEXC Feed' : 'Simulated Feed'}
          </span>
        </span>
      </div>
    </div>
  );
}

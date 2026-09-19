import { useEffect, useRef, useState } from 'react';
import { createChart, CandlestickSeries, HistogramSeries, type IChartApi, type ISeriesApi, ColorType } from 'lightweight-charts';
import './ChartsPage.css';

const TIMEFRAMES = ['1m', '5m', '15m', '30m', '1h', '4h', '1d', '1w'];
const DEMO_SYMBOLS = ['BTCUSDT', 'ETHUSDT', 'SOLUSDT', 'BNBUSDT'];

// Generate demo OHLCV data for chart display
function generateDemoCandles(count: number) {
  const candles = [];
  let price = 60000 + Math.random() * 5000;
  const now = Math.floor(Date.now() / 1000);

  for (let i = count; i > 0; i--) {
    const open = price;
    const volatility = price * 0.015;
    const close = open + (Math.random() - 0.48) * volatility;
    const high = Math.max(open, close) + Math.random() * volatility * 0.5;
    const low = Math.min(open, close) - Math.random() * volatility * 0.5;

    candles.push({
      time: (now - i * 3600) as number,
      open: parseFloat(open.toFixed(2)),
      high: parseFloat(high.toFixed(2)),
      low: parseFloat(low.toFixed(2)),
      close: parseFloat(close.toFixed(2)),
    });

    price = close;
  }
  return candles;
}

function generateDemoVolume(candles: ReturnType<typeof generateDemoCandles>) {
  return candles.map((c) => ({
    time: c.time,
    value: Math.floor(Math.random() * 5000 + 500),
    color: c.close >= c.open ? 'rgba(0, 230, 118, 0.3)' : 'rgba(255, 23, 68, 0.3)',
  }));
}

export function ChartsPage() {
  const chartContainerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | null>(null);
  const candleSeriesRef = useRef<ISeriesApi<'Candlestick'> | null>(null);
  const volumeSeriesRef = useRef<ISeriesApi<'Histogram'> | null>(null);

  const [activeSymbol, setActiveSymbol] = useState('BTCUSDT');
  const [activeTimeframe, setActiveTimeframe] = useState('1h');

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

    // Load demo data
    const candles = generateDemoCandles(200);
    candleSeries.setData(candles as any);
    volumeSeries.setData(generateDemoVolume(candles) as any);

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

  // Update chart when symbol/timeframe changes
  useEffect(() => {
    if (!candleSeriesRef.current || !volumeSeriesRef.current) return;
    const candles = generateDemoCandles(200);
    candleSeriesRef.current.setData(candles as any);
    volumeSeriesRef.current.setData(generateDemoVolume(candles) as any);
    chartRef.current?.timeScale().fitContent();
  }, [activeSymbol, activeTimeframe]);

  return (
    <div className="page charts-page animate-fade-in">
      <div className="charts-page__controls">
        {/* Symbol Selector */}
        <div className="charts-page__symbols">
          {DEMO_SYMBOLS.map((sym) => (
            <button
              key={sym}
              className={`charts-page__symbol-btn ${activeSymbol === sym ? 'charts-page__symbol-btn--active' : ''}`}
              onClick={() => setActiveSymbol(sym)}
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
          <span className="text-mono">{activeSymbol}</span>
        </span>
        <span className="charts-page__info-item">
          <span className="label">Timeframe</span>
          <span className="text-mono">{activeTimeframe}</span>
        </span>
        <span className="charts-page__info-item">
          <span className="label">Exchange</span>
          <span className="text-mono">MEXC</span>
        </span>
      </div>
    </div>
  );
}

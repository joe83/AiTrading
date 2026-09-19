// ===========================================================================
// Zustand Trading Store — Central state management for the dashboard
// ===========================================================================

import { create } from 'zustand';
import { api } from '../api/client';
import { tradingWs } from '../api/websocket';
import type {
  DashboardData,
  Position,
  Signal,
  Trade,
  BacktestResult,
  ExchangeStatus,
  SystemStatus,
  PerformanceData,
} from '../api/client';
import type { WsMessage } from '../api/websocket';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface PriceData {
  symbol: string;
  exchange: string;
  price: string;
  change_pct: number;
  timestamp: string;
}

export interface Toast {
  id: string;
  type: 'success' | 'error' | 'warning' | 'info';
  title: string;
  message: string;
  timestamp: number;
}

interface TradingState {
  // Connection
  wsConnected: boolean;

  // Dashboard data
  dashboard: DashboardData | null;
  performance: PerformanceData | null;
  systemStatus: SystemStatus | null;

  // Collections
  positions: Position[];
  signals: Signal[];
  pendingSignals: Signal[];
  trades: Trade[];
  backtestResults: BacktestResult[];
  exchanges: ExchangeStatus[];

  // Live prices
  prices: Map<string, PriceData>;

  // UI state
  toasts: Toast[];
  loading: Record<string, boolean>;

  // Actions — data fetching
  fetchDashboard: () => Promise<void>;
  fetchPositions: () => Promise<void>;
  fetchSignals: () => Promise<void>;
  fetchPendingSignals: () => Promise<void>;
  fetchTrades: () => Promise<void>;
  fetchPerformance: () => Promise<void>;
  fetchBacktestResults: () => Promise<void>;
  fetchExchanges: () => Promise<void>;
  fetchSystemStatus: () => Promise<void>;

  // Actions — trading controls
  approveSignal: (id: string) => Promise<void>;
  rejectSignal: (id: string) => Promise<void>;
  setTradingMode: (mode: 'auto' | 'manual') => Promise<void>;
  pauseTrading: () => Promise<void>;
  resumeTrading: () => Promise<void>;
  runBacktest: (config: Parameters<typeof api.runBacktest>[0]) => Promise<void>;

  // Actions — WebSocket
  initWebSocket: () => void;
  disconnectWebSocket: () => void;

  // Actions — UI
  addToast: (toast: Omit<Toast, 'id' | 'timestamp'>) => void;
  removeToast: (id: string) => void;
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

export const useTradingStore = create<TradingState>((set, get) => ({
  // Initial state
  wsConnected: false,
  dashboard: null,
  performance: null,
  systemStatus: null,
  positions: [],
  signals: [],
  pendingSignals: [],
  trades: [],
  backtestResults: [],
  exchanges: [],
  prices: new Map(),
  toasts: [],
  loading: {},

  // ---------------------------------------------------------------------------
  // Data Fetching
  // ---------------------------------------------------------------------------

  fetchDashboard: async () => {
    set((s) => ({ loading: { ...s.loading, dashboard: true } }));
    try {
      const data = await api.getDashboard();
      set({ dashboard: data, positions: data.positions, loading: { ...get().loading, dashboard: false } });
    } catch {
      set((s) => ({ loading: { ...s.loading, dashboard: false } }));
    }
  },

  fetchPositions: async () => {
    try {
      const data = await api.getPositions();
      set({ positions: data.positions });
    } catch { /* silent */ }
  },

  fetchSignals: async () => {
    try {
      const data = await api.getSignals();
      set({ signals: data.signals });
    } catch { /* silent */ }
  },

  fetchPendingSignals: async () => {
    try {
      const data = await api.getPendingSignals();
      set({ pendingSignals: data.pending_signals });
    } catch { /* silent */ }
  },

  fetchTrades: async () => {
    try {
      const data = await api.getTradeHistory();
      set({ trades: data.trades });
    } catch { /* silent */ }
  },

  fetchPerformance: async () => {
    try {
      const data = await api.getPerformance();
      set({ performance: data });
    } catch { /* silent */ }
  },

  fetchBacktestResults: async () => {
    set((s) => ({ loading: { ...s.loading, backtests: true } }));
    try {
      const data = await api.getBacktestResults();
      set({ backtestResults: data.results, loading: { ...get().loading, backtests: false } });
    } catch {
      set((s) => ({ loading: { ...s.loading, backtests: false } }));
    }
  },

  fetchExchanges: async () => {
    try {
      const data = await api.getExchangeStatus();
      set({ exchanges: data.exchanges });
    } catch { /* silent */ }
  },

  fetchSystemStatus: async () => {
    try {
      const data = await api.getSystemStatus();
      set({ systemStatus: data, exchanges: data.exchanges });
    } catch { /* silent */ }
  },

  // ---------------------------------------------------------------------------
  // Trading Controls
  // ---------------------------------------------------------------------------

  approveSignal: async (id: string) => {
    try {
      await api.approveSignal(id);
      get().addToast({ type: 'success', title: 'Signal Approved', message: `Signal ${id.slice(0, 8)} approved` });
      get().fetchPendingSignals();
      get().fetchDashboard();
    } catch {
      get().addToast({ type: 'error', title: 'Failed', message: 'Failed to approve signal' });
    }
  },

  rejectSignal: async (id: string) => {
    try {
      await api.rejectSignal(id);
      get().addToast({ type: 'warning', title: 'Signal Rejected', message: `Signal ${id.slice(0, 8)} rejected` });
      get().fetchPendingSignals();
    } catch {
      get().addToast({ type: 'error', title: 'Failed', message: 'Failed to reject signal' });
    }
  },

  setTradingMode: async (mode) => {
    try {
      await api.setTradingMode(mode);
      get().addToast({ type: 'info', title: 'Mode Changed', message: `Trading mode set to ${mode.toUpperCase()}` });
      get().fetchDashboard();
    } catch {
      get().addToast({ type: 'error', title: 'Failed', message: 'Failed to change trading mode' });
    }
  },

  pauseTrading: async () => {
    try {
      await api.pauseTrading();
      get().addToast({ type: 'warning', title: 'Trading Paused', message: 'All trading activity paused' });
      get().fetchDashboard();
    } catch {
      get().addToast({ type: 'error', title: 'Failed', message: 'Failed to pause trading' });
    }
  },

  resumeTrading: async () => {
    try {
      await api.resumeTrading();
      get().addToast({ type: 'success', title: 'Trading Resumed', message: 'Trading activity resumed' });
      get().fetchDashboard();
    } catch {
      get().addToast({ type: 'error', title: 'Failed', message: 'Failed to resume trading' });
    }
  },

  runBacktest: async (config) => {
    set((s) => ({ loading: { ...s.loading, backtest_run: true } }));
    try {
      const result = await api.runBacktest(config);
      if (result.status === 'completed') {
        get().addToast({ type: 'success', title: 'Backtest Complete', message: `Return: ${result.result?.metrics.total_return_pct.toFixed(2)}%` });
      } else {
        get().addToast({ type: 'error', title: 'Backtest Failed', message: 'Check server logs' });
      }
      get().fetchBacktestResults();
    } catch {
      get().addToast({ type: 'error', title: 'Backtest Error', message: 'Failed to run backtest' });
    } finally {
      set((s) => ({ loading: { ...s.loading, backtest_run: false } }));
    }
  },

  // ---------------------------------------------------------------------------
  // WebSocket
  // ---------------------------------------------------------------------------

  initWebSocket: () => {
    tradingWs.connect();

    tradingWs.on('connected', () => {
      set({ wsConnected: true });
    });

    tradingWs.on('*', (msg: WsMessage) => {
      switch (msg.type) {
        case 'connected':
          set({ wsConnected: true });
          break;

        case 'price_update': {
          const prices = new Map(get().prices);
          prices.set(msg.symbol, {
            symbol: msg.symbol,
            exchange: msg.exchange,
            price: msg.price,
            change_pct: msg.change_pct,
            timestamp: msg.timestamp,
          });
          set({ prices });
          break;
        }

        case 'signal':
          get().fetchPendingSignals();
          get().addToast({
            type: 'info',
            title: `AI Signal: ${msg.action}`,
            message: `${msg.symbol} — Confidence: ${(msg.confidence * 100).toFixed(0)}%`,
          });
          break;

        case 'position_update':
          get().fetchPositions();
          break;

        case 'trade_executed':
          get().addToast({
            type: 'success',
            title: 'Trade Executed',
            message: `${msg.side} ${msg.quantity} ${msg.symbol} @ ${msg.price}`,
          });
          get().fetchDashboard();
          break;

        case 'alert':
          get().addToast({
            type: msg.level === 'error' ? 'error' : msg.level === 'warning' ? 'warning' : 'info',
            title: 'Alert',
            message: msg.message,
          });
          break;

        case 'system_status':
          set((s) => ({
            systemStatus: s.systemStatus
              ? { ...s.systemStatus, trading_mode: msg.trading_mode, is_paused: msg.is_paused, open_positions: msg.open_positions }
              : null,
          }));
          break;
      }
    });
  },

  disconnectWebSocket: () => {
    tradingWs.disconnect();
    set({ wsConnected: false });
  },

  // ---------------------------------------------------------------------------
  // UI
  // ---------------------------------------------------------------------------

  addToast: (toast) => {
    const id = crypto.randomUUID();
    set((s) => ({
      toasts: [...s.toasts, { ...toast, id, timestamp: Date.now() }],
    }));
    // Auto-remove after 5 seconds
    setTimeout(() => {
      get().removeToast(id);
    }, 5000);
  },

  removeToast: (id) => {
    set((s) => ({
      toasts: s.toasts.filter((t) => t.id !== id),
    }));
  },
}));

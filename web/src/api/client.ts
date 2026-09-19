// ===========================================================================
// REST API Client — Type-safe wrapper for all Rust server endpoints
// ===========================================================================

const API_BASE = import.meta.env.VITE_API_URL || 'http://localhost:8080';

// ---------------------------------------------------------------------------
// Types matching Rust server responses
// ---------------------------------------------------------------------------

export interface DashboardData {
  open_positions: number;
  pending_signals: number;
  total_trades: number;
  unrealized_pnl: number;
  win_rate: number;
  trading_mode: string;
  is_paused: boolean;
  positions: Position[];
}

export interface Position {
  id: string;
  symbol: string;
  exchange: string;
  side: string;
  entry_price: string;
  quantity: string;
  unrealized_pnl: string;
  stop_loss: string | null;
  take_profit: string | null;
  opened_at: string;
}

export interface Signal {
  id: string;
  symbol: string;
  exchange: string;
  action: string;
  confidence: number;
  reasoning: string;
  timestamp: string;
  status?: string;
}

export interface Trade {
  id: string;
  symbol: string;
  exchange: string;
  side: string;
  entry_price: string;
  exit_price: string | null;
  quantity: string;
  realized_pnl: string | null;
  opened_at: string;
  closed_at: string | null;
}

export interface PerformanceData {
  total_trades: number;
  closed_trades: number;
  wins: number;
  losses: number;
  win_rate: number;
}

export interface Analysis {
  symbol: string;
  recommendation: string;
  confidence: number;
  reasoning: string;
  indicators: Record<string, unknown>;
  timestamp: string;
}

export interface BacktestConfig {
  symbol: string;
  timeframe: string;
  start_time: string;
  end_time: string;
  initial_capital: number;
  fee_rate: number;
  stop_loss_pct: number;
  take_profit_pct: number;
}

export interface BacktestMetrics {
  total_return_pct: number;
  total_trades: number;
  win_rate_pct: number;
  sharpe_ratio: number;
  sortino_ratio: number;
  max_drawdown_pct: number;
  profit_factor: number;
  avg_win_pct: number;
  avg_loss_pct: number;
}

export interface BacktestResult {
  id: string;
  config: BacktestConfig;
  metrics: BacktestMetrics;
  trades: BacktestTrade[];
  equity_curve: number[];
  completed_at: string;
}

export interface BacktestTrade {
  entry_time: string;
  exit_time: string;
  side: string;
  entry_price: number;
  exit_price: number;
  quantity: number;
  pnl: number;
  pnl_pct: number;
}

export interface ExchangeStatus {
  exchange: string;
  connected: boolean;
}

export interface SystemStatus {
  server: string;
  trading_mode: string;
  is_paused: boolean;
  open_positions: number;
  grok_tokens_used: number;
  uptime_secs: number;
  exchanges: ExchangeStatus[];
}

// ---------------------------------------------------------------------------
// API Client
// ---------------------------------------------------------------------------

async function request<T>(path: string, options?: RequestInit, skipAuth = false): Promise<T> {
  const url = `${API_BASE}${path}`;

  // Build headers with auth token
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    ...(options?.headers as Record<string, string>),
  };

  // Inject JWT token if available (skip for login/health endpoints)
  if (!skipAuth) {
    const token = localStorage.getItem('aitrading_token');
    if (token) {
      headers['Authorization'] = `Bearer ${token}`;
    }
  }

  const res = await fetch(url, {
    ...options,
    headers,
  });

  if (res.status === 401 && !skipAuth) {
    // Token expired — clear and redirect to login
    localStorage.removeItem('aitrading_token');
    localStorage.removeItem('aitrading_user');
    window.location.href = '/login';
    throw new Error('Session expired. Please log in again.');
  }

  if (!res.ok) {
    throw new Error(`API error: ${res.status} ${res.statusText}`);
  }

  return res.json();
}

export const api = {
  // Dashboard
  getDashboard: () =>
    request<DashboardData>('/api/dashboard'),

  // Positions
  getPositions: () =>
    request<{ positions: Position[] }>('/api/positions'),

  // Analysis
  getAnalysis: (symbol: string) =>
    request<{ analysis: Analysis | null }>(`/api/analysis/${symbol}`),

  // Signals
  getSignals: () =>
    request<{ signals: Signal[] }>('/api/signals'),

  getPendingSignals: () =>
    request<{ pending_signals: Signal[] }>('/api/signals/pending'),

  approveSignal: (id: string) =>
    request<{ status: string }>(`/api/signals/${id}/approve`, { method: 'POST' }),

  rejectSignal: (id: string) =>
    request<{ status: string }>(`/api/signals/${id}/reject`, { method: 'POST' }),

  // Orders
  placeOrder: (order: {
    symbol: string;
    side: string;
    order_type: string;
    quantity: string;
    price?: string;
  }) =>
    request<{ status: string }>('/api/orders', {
      method: 'POST',
      body: JSON.stringify(order),
    }),

  cancelOrder: (symbol: string, orderId: string) =>
    request<{ status: string }>(`/api/orders/${symbol}/${orderId}`, {
      method: 'DELETE',
    }),

  // Performance
  getPerformance: () =>
    request<PerformanceData>('/api/performance'),

  getTradeHistory: () =>
    request<{ trades: Trade[] }>('/api/trades'),

  // Backtesting
  runBacktest: (config: BacktestConfig) =>
    request<{ status: string; result?: BacktestResult }>('/api/backtest/run', {
      method: 'POST',
      body: JSON.stringify(config),
    }),

  getBacktestResults: () =>
    request<{ results: BacktestResult[] }>('/api/backtest/results'),

  getBacktestResult: (id: string) =>
    request<{ result: BacktestResult }>(`/api/backtest/${id}`),

  // Exchanges
  getExchangeStatus: () =>
    request<{ exchanges: ExchangeStatus[] }>('/api/exchanges/status'),

  // Settings & Control
  getTradingMode: () =>
    request<{ mode: string }>('/api/settings/mode'),

  setTradingMode: (mode: 'auto' | 'manual') =>
    request<{ mode: string }>('/api/settings/mode', {
      method: 'POST',
      body: JSON.stringify({ mode }),
    }),

  pauseTrading: () =>
    request<{ status: string }>('/api/control/pause', { method: 'POST' }),

  resumeTrading: () =>
    request<{ status: string }>('/api/control/resume', { method: 'POST' }),

  // System
  healthCheck: () =>
    request<{ status: string; timestamp: string }>('/api/health'),

  getSystemStatus: () =>
    request<SystemStatus>('/api/status'),

  // Authentication
  login: (username: string, password: string) =>
    request<{ token: string; expires_in: number; token_type: string }>(
      '/api/auth/login',
      {
        method: 'POST',
        body: JSON.stringify({ username, password }),
      },
      true, // skip auth header for login
    ),

  verifyToken: () =>
    request<{ valid: boolean; username: string; expires_at: number }>('/api/auth/verify'),
};

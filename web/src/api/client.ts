// ===========================================================================
// REST API Client — Type-safe wrapper for all Rust server endpoints
// ===========================================================================

const API_BASE = import.meta.env.PROD ? '' : (import.meta.env.VITE_API_URL ?? '');

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
  use_grok?: boolean;
  max_ai_calls?: number;
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
  use_grok?: boolean;
  ai_calls_made?: number;
  ai_cached_calls?: number;
  grok_model_used?: string;
  completed_at: string;
}

export interface BacktestTrade {
  entry_time: string;
  exit_time: string;
  symbol?: string;
  side: string;
  entry_price: number;
  exit_price: number;
  quantity: number;
  pnl: number;
  pnl_pct: number;
  fees?: number;
  holding_bars?: number;
  reasoning?: string;
  exit_reason?: string;
}

export interface ExchangeStatus {
  exchange: string;
  id?: string;
  connected: boolean;
  enabled?: boolean;
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

export interface GrokKeyStatus {
  is_set: boolean;
  masked_key: string;
  base_url: string;
  model_primary: string;
  model_fast: string;
  mode?: 'api' | 'proxy' | string;
  proxy_account?: string | null;
}

export interface MexcKeyStatus {
  is_set: boolean;
  masked_api_key: string;
  is_secret_set: boolean;
  base_url: string;
  enabled?: boolean;
}

export interface AlpacaKeyStatus {
  is_set: boolean;
  masked_api_key: string;
  is_secret_set: boolean;
  base_url: string;
  enabled?: boolean;
}

export interface IcMarketsKeyStatus {
  is_set: boolean;
  masked_api_key: string;
  account_id: string;
  client_id: string;
  is_client_secret_set: boolean;
  base_url: string;
  enabled?: boolean;
}

export interface BinanceKeyStatus {
  is_set: boolean;
  masked_api_key: string;
  is_secret_set: boolean;
  base_url: string;
  enabled?: boolean;
}

export interface BybitKeyStatus {
  is_set: boolean;
  masked_api_key: string;
  is_secret_set: boolean;
  base_url: string;
  enabled?: boolean;
}

export interface ApiKeysStatus {
  grok: GrokKeyStatus;
  mexc: MexcKeyStatus;
  alpaca: AlpacaKeyStatus;
  ic_markets: IcMarketsKeyStatus;
  binance: BinanceKeyStatus;
  bybit: BybitKeyStatus;
}

export interface UpdateApiKeysRequest {
  grok_api_key?: string;
  grok_base_url?: string;
  grok_mode?: 'api' | 'proxy';
  grok_model_primary?: string;
  grok_model_fast?: string;

  mexc_api_key?: string;
  mexc_secret_key?: string;
  mexc_enabled?: boolean;

  alpaca_api_key?: string;
  alpaca_secret_key?: string;
  alpaca_base_url?: string;
  alpaca_enabled?: boolean;

  ic_markets_api_key?: string;
  ic_markets_account_id?: string;
  ic_markets_client_id?: string;
  ic_markets_client_secret?: string;
  ic_markets_enabled?: boolean;

  binance_api_key?: string;
  binance_secret_key?: string;
  binance_base_url?: string;
  binance_enabled?: boolean;

  bybit_api_key?: string;
  bybit_secret_key?: string;
  bybit_base_url?: string;
  bybit_enabled?: boolean;
}

export interface TestApiKeyRequest {
  service: 'grok' | 'mexc' | 'alpaca' | 'ic_markets' | 'binance' | 'bybit';
  key?: string;
  secret?: string;
}

export interface TestApiKeyResponse {
  success: boolean;
  message: string;
}

export interface MemeTokenRadarItem {
  symbol: string;
  name: string;
  cashtag: string;
  narrative: string;
  viral_velocity: number;
  sentiment_score: number;
  sentiment_label: string;
  catalysts: string[];
  risk_level: 'low' | 'medium' | 'high' | 'extreme';
  is_tradeable_on_mexc: boolean;
  mexc_symbol: string;
  current_price_usdt?: number | null;
  price_change_24h_pct?: number | null;
  volume_24h_usdt?: number | null;
  high_24h?: number | null;
  low_24h?: number | null;
}

export interface MemeRadarReport {
  scanned_at: string;
  total_tokens_scanned: number;
  top_narrative_theme: string;
  narrative_summary: string;
  tokens: MemeTokenRadarItem[];
  source: string;
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

  runBacktest: (config: BacktestConfig) =>
    request<{ status: string; result?: BacktestResult }>('/api/backtest/run', {
      method: 'POST',
      body: JSON.stringify({
        ...config,
        exchange: 'mexc',
        initial_balance: config.initial_capital,
        maker_fee: config.fee_rate,
        taker_fee: config.fee_rate,
      }),
    }),

  getBacktestResults: () =>
    request<{ results: BacktestResult[] }>('/api/backtest/results'),

  getBacktestResult: (id: string) =>
    request<{ result: BacktestResult }>(`/api/backtest/${id}`),

  // Exchanges
  getExchangeStatus: () =>
    request<{ exchanges: ExchangeStatus[] }>('/api/exchanges/status'),

  toggleExchange: (exchange: string, enabled?: boolean) =>
    request<{ status: string; exchange: string; connected: boolean; enabled: boolean }>(
      `/api/exchanges/${encodeURIComponent(exchange.toLowerCase())}/toggle`,
      {
        method: 'POST',
        body: JSON.stringify({ enabled }),
      }
    ),

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

  // API Keys & Integrations
  getApiKeys: () =>
    request<ApiKeysStatus>('/api/settings/api-keys'),

  updateApiKeys: (data: UpdateApiKeysRequest) =>
    request<{ success: boolean; message: string }>('/api/settings/api-keys', {
      method: 'POST',
      body: JSON.stringify(data),
    }),

  testApiKey: (data: TestApiKeyRequest) =>
    request<TestApiKeyResponse>('/api/settings/api-keys/test', {
      method: 'POST',
      body: JSON.stringify(data),
    }),

  // Meme / Social Sentiment Radar
  getMemeRadar: () =>
    request<{ report: MemeRadarReport }>('/api/radar/memes'),

  scanMemeRadar: () =>
    request<{ success: boolean; report: MemeRadarReport }>('/api/radar/memes/scan', {
      method: 'POST',
    }),

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

  getLessons: (status?: string) => {
    const query = status ? `?status=${encodeURIComponent(status)}` : '';
    return request<{ lessons: LessonRecord[] }>(`/api/lessons${query}`);
  },

  approveLesson: (id: string) =>
    request<LessonDecision>(`/api/lessons/${id}/approve`, { method: 'POST' }),

  rejectLesson: (id: string) =>
    request<{ id: string; status: string }>(`/api/lessons/${id}/reject`, { method: 'POST' }),

  getWatch: () => request<WatchSnapshot>('/api/watch'),

  getGrokAccount: () => request<GrokAccount>('/api/grok/account'),

  getGrokLogin: () => request<GrokLogin>('/api/grok/login'),

  startGrokLogin: () => request<GrokLogin>('/api/grok/login', { method: 'POST' }),

  // Market Candlesticks
  getMarketCandles: (symbol: string, timeframe = '1h', limit = 200) =>
    request<CandleData[]>(
      `/api/market/candles?symbol=${encodeURIComponent(symbol)}&timeframe=${encodeURIComponent(timeframe)}&limit=${limit}`,
    ),
};

export interface CandleData {
  time: number;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
}

export interface LessonRecord {
  id: string;
  scope: string;
  claim: string;
  evidence_trade_ids: string[];
  sample_size: number;
  status: 'hypothesis' | 'supported' | 'rejected' | string;
  created_at: string;
}

export interface WatchSnapshot {
  status: {
    enabled: boolean;
    handles: string[];
    interval_secs: number;
    running: boolean;
    last_tick_at: string | null;
    last_result: string;
    last_error: string | null;
  };
  conductor?: {
    running: boolean;
    last_run_at: string | null;
    last_result: string;
    last_error: string | null;
  };
  posts: Array<{
    post_id: string;
    handle: string;
    body: string;
    queued: boolean;
    seen_at: string;
  }>;
}

export interface GrokAccount {
  logged_in: boolean;
  account: string | null;
  error?: string;
}

export interface GrokLogin {
  status: 'idle' | 'waiting' | 'approved' | 'error' | string;
  verification_url?: string | null;
  user_code?: string | null;
  account?: string | null;
  error?: string | null;
}

export interface LessonDecision {
  lesson_id: string;
  id: string;
  scope: string;
  rule: string;
  sample_size: number;
  status: string;
}

export const apiClient = api;


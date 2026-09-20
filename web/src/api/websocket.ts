// ===========================================================================
// WebSocket Client — Auto-reconnecting connection to the trading server
// ===========================================================================

export type WsEventType =
  | 'price_update'
  | 'signal'
  | 'position_update'
  | 'trade_executed'
  | 'alert'
  | 'system_status'
  | 'connected';

export interface WsPriceUpdate {
  type: 'price_update';
  symbol: string;
  exchange: string;
  price: string;
  change_pct: number;
  timestamp: string;
}

export interface WsSignal {
  type: 'signal';
  symbol: string;
  action: string;
  confidence: number;
  reasoning: string;
  timestamp: string;
}

export interface WsPositionUpdate {
  type: 'position_update';
  symbol: string;
  side: string;
  pnl: string;
  pnl_pct: number;
  timestamp: string;
}

export interface WsTradeExecuted {
  type: 'trade_executed';
  symbol: string;
  side: string;
  quantity: string;
  price: string;
  timestamp: string;
}

export interface WsAlert {
  type: 'alert';
  level: string;
  message: string;
  timestamp: string;
}

export interface WsSystemStatus {
  type: 'system_status';
  trading_mode: string;
  is_paused: boolean;
  open_positions: number;
  timestamp: string;
}

export type WsMessage =
  | WsPriceUpdate
  | WsSignal
  | WsPositionUpdate
  | WsTradeExecuted
  | WsAlert
  | WsSystemStatus
  | { type: 'connected'; message: string; timestamp: string };

type WsListener = (msg: WsMessage) => void;

function getDefaultWsUrl(): string {
  if (import.meta.env.VITE_WS_URL) {
    const url = import.meta.env.VITE_WS_URL;
    return url.endsWith('/ws') ? url : `${url}/ws`;
  }
  if (typeof window !== 'undefined') {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    return `${protocol}//${window.location.host}/ws`;
  }
  return 'ws://localhost:8081/ws';
}

export class TradingWebSocket {
  private ws: WebSocket | null = null;
  private listeners: Map<WsEventType | '*', Set<WsListener>> = new Map();
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 20;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private pingTimer: ReturnType<typeof setInterval> | null = null;
  private _isConnected = false;
  private _url: string;

  constructor() {
    this._url = getDefaultWsUrl();
  }

  get isConnected(): boolean {
    return this._isConnected;
  }

  connect(): void {
    if (this.ws?.readyState === WebSocket.OPEN) return;

    try {
      this.ws = new WebSocket(this._url);

      this.ws.onopen = () => {
        this._isConnected = true;
        this.reconnectAttempts = 0;
        this.startPingInterval();
        this.emit({ type: 'connected', message: 'WebSocket connected', timestamp: new Date().toISOString() });
      };

      this.ws.onmessage = (event) => {
        try {
          const msg: WsMessage = JSON.parse(event.data);
          this.emit(msg);
        } catch {
          console.warn('WS: failed to parse message', event.data);
        }
      };

      this.ws.onclose = () => {
        this._isConnected = false;
        this.stopPingInterval();
        this.scheduleReconnect();
      };

      this.ws.onerror = () => {
        this._isConnected = false;
      };
    } catch {
      this.scheduleReconnect();
    }
  }

  disconnect(): void {
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    this.stopPingInterval();
    this.reconnectAttempts = this.maxReconnectAttempts; // prevent reconnect
    this.ws?.close();
    this.ws = null;
    this._isConnected = false;
  }

  on(event: WsEventType | '*', listener: WsListener): () => void {
    if (!this.listeners.has(event)) {
      this.listeners.set(event, new Set());
    }
    this.listeners.get(event)!.add(listener);

    // Return unsubscribe function
    return () => {
      this.listeners.get(event)?.delete(listener);
    };
  }

  subscribe(symbols: string[]): void {
    this.send({ action: 'subscribe', symbols });
  }

  private emit(msg: WsMessage): void {
    // Emit to specific event listeners
    this.listeners.get(msg.type as WsEventType)?.forEach((fn) => fn(msg));
    // Emit to wildcard listeners
    this.listeners.get('*')?.forEach((fn) => fn(msg));
  }

  private send(data: unknown): void {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(data));
    }
  }

  private scheduleReconnect(): void {
    if (this.reconnectAttempts >= this.maxReconnectAttempts) return;

    const delay = Math.min(1000 * Math.pow(2, this.reconnectAttempts), 30000);
    this.reconnectAttempts++;

    this.reconnectTimer = setTimeout(() => {
      this.connect();
    }, delay);
  }

  private startPingInterval(): void {
    this.pingTimer = setInterval(() => {
      this.send({ action: 'ping' });
    }, 30000);
  }

  private stopPingInterval(): void {
    if (this.pingTimer) {
      clearInterval(this.pingTimer);
      this.pingTimer = null;
    }
  }
}

// Singleton instance
export const tradingWs = new TradingWebSocket();

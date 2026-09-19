# ⚡ AI Trading Platform

An ultra-low latency, multi-market algorithmic trading platform powered by **Rust**, **Grok AI (xAI)**, **TimescaleDB**, **React 19**, and **Flutter**.

Designed for crypto, US equities, and forex trading with real-time autonomous analysis, automated risk management, desktop web dashboard, and mobile companion app.

---

## 🌟 Key Features

- **Multi-Market Engine**: Native exchange connectors for **MEXC** (Crypto), **Alpaca** (US Stocks), and **IC Markets** (Forex).
- **Grok AI Synthesis**: Real-time multi-signal analysis combining technical indicators (RSI, MACD, Bollinger Bands, ATR), market sentiment, and price action patterns into actionable trading decisions.
- **Strict Risk Management**: Real-time position sizing, daily drawdown limits, stop-loss/take-profit enforcement, and emergency kill-switch controls.
- **High-Performance Time-Series Data**: TimescaleDB hypertables with automated compression and continuous aggregates.
- **Modern Web Dashboard**: Desktop-class React 19 + TypeScript dashboard featuring TradingView candlestick charts, order execution, real-time WebSocket feeds, and backtesting runner.
- **Flutter Mobile App**: Native Android/iOS companion application for on-the-go portfolio monitoring and signal approvals.
- **Enterprise Security**: Single-user JWT authentication, SHA-256 hashed credentials, Axum route protection middleware, and Nginx brute-force rate-limiting.

---

## 🚀 Quick Start & Production Installation

### 1-Click Production Installer (Ubuntu / Debian)

For production deployment on a VPS (Hetzner, AWS, DigitalOcean, Vultr):

```bash
git clone https://github.com/your-username/AiTrading.git /opt/aitrading
cd /opt/aitrading
chmod +x install.sh scripts/*.sh
sudo ./install.sh
```

The installer automatically:
- Installs Docker Engine, Compose plugin, UFW, and Fail2ban.
- Configures firewall rules (securing DB & internal ports, opening only 22, 80, 443).
- Generates cryptographically secure credentials for TimescaleDB, Redis, and JWT.
- Configures Nginx reverse proxy with WebSocket stream support and rate-limited auth.
- Builds and starts production containers in detached mode.
- Sets up automated daily backups via cron.

👉 **Read the full [Production Deployment Guide](docs/DEPLOYMENT.md)** for hardware sizing, region selection, and SSL configuration.

---

## 🛠️ Local Development Setup

### Prerequisites
- **Rust** 1.75+ (`rustup default stable`)
- **Node.js** 20+ & `npm`
- **Docker** (for local TimescaleDB & Redis)

### 1. Start Supporting Services
```bash
docker-compose up -d
```
This launches TimescaleDB on `localhost:5432` and Redis on `localhost:6379`.

### 2. Configure & Run Backend Server
```bash
cd server
cp .env.example .env
# Fill in your XAI_API_KEY from console.x.ai in server/.env
cargo run
```
The server will boot on:
- **REST API**: `http://localhost:8080`
- **WebSocket**: `ws://localhost:8081`

### 3. Run Web Dashboard
```bash
cd ../web
npm install
npm run dev
```
Open [http://localhost:5173](http://localhost:5173) in your browser.
Default credentials:
- **Username**: `admin`
- **Password**: `admin123`

---

## 📡 REST API & WebSocket Reference

### Public Endpoints
| Method | Endpoint | Description |
| :--- | :--- | :--- |
| `GET` | `/api/health` | System health check & timestamp |
| `POST` | `/api/auth/login` | JWT login (`{ username, password }`) |

### Protected Endpoints (Requires `Authorization: Bearer <token>`)
| Method | Endpoint | Description |
| :--- | :--- | :--- |
| `GET` | `/api/auth/verify` | Validate current JWT session |
| `GET` | `/api/dashboard` | Portfolio metrics, open positions, recent signals |
| `GET` | `/api/positions` | List open positions with live P&L |
| `GET` | `/api/signals` | AI trading signal history |
| `GET` | `/api/signals/pending` | Signals awaiting manual approval |
| `POST` | `/api/signals/:id/approve` | Approve signal for immediate execution |
| `POST` | `/api/signals/:id/reject` | Reject signal |
| `GET` | `/api/analysis/:symbol` | Latest Grok AI technical/sentiment report |
| `POST` | `/api/orders` | Place manual order |
| `DELETE` | `/api/orders/:symbol/:id` | Cancel open order |
| `GET` | `/api/performance` | Win rate, profit factor, drawdown metrics |
| `GET` | `/api/trades` | Historical executed trades |
| `POST` | `/api/backtest/run` | Execute historical strategy backtest |
| `GET` | `/api/backtest/results` | List past backtest results |
| `GET` | `/api/exchanges/status` | Exchange connectivity & latency |
| `GET` | `/api/settings/mode` | Current trading mode (`auto` / `manual`) |
| `POST` | `/api/settings/mode` | Toggle trading mode |
| `POST` | `/api/control/pause` | Emergency kill-switch (pauses trading) |
| `POST` | `/api/control/resume` | Resume active trading |

### Real-Time WebSocket Feed
Connect to `ws://localhost:8081/ws` (or `wss://yourdomain.com/ws` in production) for live feeds:
- `price_update`: Live ticker quotes & 24h change
- `signal_generated`: Real-time AI trading opportunities
- `position_update`: Live unrealized P&L and status
- `order_executed`: Exchange execution notifications
- `system_status`: Engine heartbeats and risk warnings

---

## 📁 Repository Structure

```
AiTrading/
├── install.sh                  # One-click production installer
├── docker-compose.yml          # Local development compose (DB + Redis)
├── docker-compose.prod.yml     # Production multi-tier compose stack
├── docs/
│   └── DEPLOYMENT.md           # Production deployment & operations guide
├── docker/
│   └── nginx/                  # Production reverse proxy configs & certs
├── scripts/
│   ├── deploy.sh               # Automated deployment script
│   └── backup.sh               # Automated TimescaleDB backup & retention
├── server/                     # Rust backend trading engine
│   ├── Dockerfile              # Multi-stage production container build
│   ├── Cargo.toml
│   └── src/
│       ├── ai/                 # Grok AI, technical indicators, pattern rec.
│       ├── api/                # Axum REST API & WebSocket handlers
│       ├── auth.rs             # JWT authentication & middleware
│       ├── exchange/           # MEXC, Alpaca, IC Markets connectors
│       ├── execution/          # Order routing & state machine
│       ├── models/             # Shared data models & types
│       └── risk/               # Position sizing & risk guardrails
├── web/                        # React 19 + TypeScript desktop dashboard
│   ├── Dockerfile              # Multi-stage production container build
│   ├── src/                    # Components, pages, stores, TradingView charts
│   └── styles/                 # Cyberpunk/dark theme design system
└── mobile/                     # Flutter Android & iOS companion app
```

---

## 🛡️ License

Private & Proprietary. All rights reserved.

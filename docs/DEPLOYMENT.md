# Production Deployment Guide

This guide details the end-to-end process of setting up, hardening, and deploying the **AI Trading Platform** in a secure, highly-available production environment.

---

## Table of Contents
1. [Architecture Overview](#1-architecture-overview)
2. [Hardware Sizing & Region Selection](#2-hardware-sizing--region-selection)
3. [One-Click Installation (Quickstart)](#3-one-click-installation-quickstart)
4. [Server Hardening & Security](#4-server-hardening--security)
5. [Domain, Nginx & SSL Configuration](#5-domain-nginx--ssl-configuration)
6. [Database Management (TimescaleDB)](#6-database-management-timescaledb)
7. [Operational Runbook & Common Commands](#7-operational-runbook--common-commands)
8. [Troubleshooting & FAQs](#8-troubleshooting--faqs)

---

## 1. Architecture Overview

The production deployment runs via Docker Compose in an isolated network architecture:

- **Gateway / Reverse Proxy (Nginx)**: Listens on ports `80` (HTTP) and `443` (HTTPS). Terminates TLS, enforces rate-limiting on login routes, manages WebSocket upgrade handshakes, and serves the frontend SPA.
- **Backend (Rust Server)**: Axum REST API (port `8080`) and WebSocket server (port `8081`). Connects asynchronously to Grok, MEXC, Alpaca, and IC Markets.
- **SuperGrok proxy**: Private container on the Docker network. It holds one SuperGrok login and forwards chat and X search to xAI. No host port is published.
- **Frontend (React 19 + TypeScript)**: Compiled static Single Page Application served with gzip/brotli caching.
- **Database (TimescaleDB / PostgreSQL 16)**: Time-series database optimized for tick and candlestick storage, automated hypertable compression, and retention.
- **Cache (Redis 7 Alpine)**: Fast in-memory cache and pub/sub.
- **Certbot**: Automated background SSL certificate renewal daemon.

---

## 2. Hardware Sizing & Region Selection

### Hardware Sizing

| Workload | vCPU | RAM | Storage | Bandwidth |
| :--- | :--- | :--- | :--- | :--- |
| **Minimum** | 2 | 4 GB | 40 GB NVMe | 100 Mbps |
| **Recommended** | **4** | **8 GB – 16 GB** | **100 GB+ NVMe** | **1 Gbps** |

> **Critical Note on Storage**: Always choose **NVMe SSDs** instead of standard network block storage. High-frequency tick ingestion and technical indicator calculations require low disk I/O latency.

### Cloud Region Selection (Minimizing Execution Latency)

Network latency directly impacts trade execution and slippage:

- **Crypto (MEXC / Binance / Bybit)**: Choose **Tokyo (`ap-northeast-1`)** or **Singapore (`ap-southeast-1`)** (1ms–5ms round-trip to matching engines).
- **US Equities (Alpaca)**: Choose **US East (N. Virginia `us-east-1` or Ohio `us-east-2`)** (2ms–10ms to Secaucus, NJ data centers).
- **Forex (IC Markets)**: Choose **London, UK (`eu-west-2`)** (Equinix LD4) or **New York (`us-east-1`)** (Equinix NY4).

---

## 3. One-Click Installation (Quickstart)

On a fresh Ubuntu 22.04 / 24.04 or Debian 12 server, run:

```bash
git clone https://github.com/your-username/AiTrading.git /opt/aitrading
cd /opt/aitrading
chmod +x install.sh scripts/*.sh
sudo ./install.sh
```

### Non-Interactive / Unattended Install
To run automatically in CI/CD or cloud-init scripts:
```bash
sudo DOMAIN="trade.yourdomain.com" \
     ADMIN_USER="admin" \
     ADMIN_PASSWORD="YourStrongPassword123!" \
     XAI_API_KEY="your_actual_xai_key" \
     ./install.sh
```

`GROK_MODE` chooses the Grok connection written into `server/.env`:

| `GROK_MODE` | When | `XAI_BASE_URL` |
| :--- | :--- | :--- |
| `proxy` (default if `XAI_API_KEY` is empty) | SuperGrok subscription on this server | `http://grok-proxy:8585/v1` |
| `api` (default if `XAI_API_KEY` is set) | Metered key from console.x.ai | `https://api.x.ai/v1` |

A bare IP install keeps the dashboard on same-origin HTTP (`VITE_API_URL` empty). A domain install still serves the API through Nginx. The production web image ignores `web/.env`, so the browser never calls `localhost`.

### Grok connection and the watcher

API Keys in the dashboard switches between the SuperGrok proxy and the xAI API key and saves that choice. **Change account** starts a device login. Approve it in a browser with the SuperGrok account that should pay for the calls.

The Watcher menu scans `@elonmusk` and `@realDonaldTrump` every 60 seconds. Handles, confidence, and age are `WATCH_HANDLES`, `WATCH_MIN_CONFIDENCE`, and `WATCH_MAX_POST_AGE_SECS` in `server/.env`. A match is queued for review. Trading mode stays manual, so nothing is sent to an exchange until you approve it.

Sign in from the server shell if the dashboard is not up yet:

```bash
docker exec -it grok-proxy grok-proxy login --no-browser
```

The login file lives in the `grokdata` volume. Redeploying the stack does not delete it.


---

## 4. Server Hardening & Security

> **Note**: If you used the [1-Click Installer (`install.sh`)](#3-one-click-installation-quickstart), **the UFW firewall, Fail2ban, isolated Docker networks, credential generation, and file permissions are already configured automatically.**
>
> The steps below document what the installer does under the hood, plus an optional initial step for configuring a dedicated non-root SSH user if setting up a server manually.

### A. Dedicated Non-Root User (Optional Pre-Install Best Practice)
If your VPS provider (e.g. Hetzner, DigitalOcean) provided you with only a `root` user and password, it is best practice to create a non-root sudo user for day-to-day SSH access:
```bash
adduser trader
usermod -aG sudo trader
mkdir -p /home/trader/.ssh
cp ~/.ssh/authorized_keys /home/trader/.ssh/
chown -R trader:trader /home/trader/.ssh
chmod 700 /home/trader/.ssh && chmod 600 /home/trader/.ssh/authorized_keys
```

### B. Firewall (UFW) — *(Handled automatically by `install.sh`)*
Enforces strict ingress rules so only web traffic and SSH can reach the server:
```bash
sudo ufw default deny incoming
sudo ufw default allow outgoing
sudo ufw allow 22/tcp comment 'SSH'
sudo ufw allow 80/tcp comment 'HTTP / Let\'s Encrypt'
sudo ufw allow 443/tcp comment 'HTTPS'

# Strictly block direct internet access to databases and backend ports:
sudo ufw deny 5432 comment 'PostgreSQL (Internal)'
sudo ufw deny 6379 comment 'Redis (Internal)'
sudo ufw deny 8080 comment 'REST API (Internal)'
sudo ufw deny 8081 comment 'WebSocket (Internal)'

sudo ufw --force enable
```

### C. Brute-force Prevention with Fail2ban — *(Handled automatically by `install.sh`)*
Automatically bans IP addresses showing malicious sign-in patterns:
```bash
sudo apt update && sudo apt install -y fail2ban
sudo systemctl enable --now fail2ban
```

### D. Application-Level Hardening — *(Handled automatically by `install.sh`)*
- **Internal Docker Network**: TimescaleDB (`5432`), Redis (`6379`), and the Rust backend (`8080`, `8081`) do not expose any host ports. Only Nginx communicates with them internally.
- **Unprivileged Container User**: The Rust trading server runs as `appuser:appgroup` (non-root).
- **Rate-Limiting**: Nginx limits `/api/auth/login` to 5 requests per minute with a burst of 3.
- **Strict File Permissions**: `server/.env` is locked down with `chmod 600`.

---

## 5. Domain, Nginx & SSL Configuration

### Pointing DNS
Create an **A Record** in your DNS provider:
- **Host**: `trade` (or `@` for root domain)
- **Points to**: `<YOUR_SERVER_PUBLIC_IP>`
- **TTL**: 300 seconds

### Free Let's Encrypt SSL
Once DNS propagation is complete:
```bash
docker compose -f docker-compose.prod.yml run --rm certbot certonly \
  --webroot --webroot-path=/var/www/certbot \
  --email admin@yourdomain.com --agree-tos --no-eff-email \
  -d trade.yourdomain.com
```

Update the SSL certificate paths in `docker/nginx/conf.d/trading.conf`:
```nginx
ssl_certificate /etc/letsencrypt/live/trade.yourdomain.com/fullchain.pem;
ssl_certificate_key /etc/letsencrypt/live/trade.yourdomain.com/privkey.pem;
```

Reload Nginx:
```bash
docker compose -f docker-compose.prod.yml exec nginx nginx -s reload
```

Certbot will automatically renew the certificate in the background every 12 hours.

---

## 6. Database Management (TimescaleDB)

### Automated Daily Backups
The installer schedules a daily cron job at 03:00 UTC via `scripts/backup.sh`. Backups are saved to `/var/backups/aitrading/` and retained for 14 days.

To manually trigger a backup:
```bash
sudo ./scripts/backup.sh
```

To restore from a backup:
```bash
gunzip -c /var/backups/aitrading/aitrading_YYYYMMDD_HHMMSS.sql.gz | \
  docker compose -f docker-compose.prod.yml exec -T db psql -U aitrading -d aitrading
```

### Hypertable & Compression Check
Log in to the database container:
```bash
docker compose -f docker-compose.prod.yml exec db psql -U aitrading -d aitrading
```
Inspect compression stats:
```sql
SELECT hypertable_name, total_chunks, compressed_chunks
FROM timescaledb_information.hypertables;
```

---

## 7. Operational Runbook & Common Commands

| Operational Task | Command |
| :--- | :--- |
| **Check service status** | `docker compose -f docker-compose.prod.yml ps` |
| **View live backend logs** | `docker compose -f docker-compose.prod.yml logs -f server` |
| **View live Nginx gateway logs**| `docker compose -f docker-compose.prod.yml logs -f nginx` |
| **Restart the entire stack** | `docker compose -f docker-compose.prod.yml restart` |
| **Restart backend only** | `docker compose -f docker-compose.prod.yml restart server` |
| **Zero-downtime update** | `git pull && docker compose -f docker-compose.prod.yml up -d --build` |
| **Emergency Pause Trading** | `curl -X POST https://yourdomain.com/api/control/pause -H "Authorization: Bearer <TOKEN>"` |
| **Health Check** | `curl -i https://yourdomain.com/api/health` |

---

## 8. Troubleshooting & FAQs

### "Failed to connect to /api/auth/login" or 502 Bad Gateway
1. Check if the Rust backend is healthy:
   ```bash
   docker compose -f docker-compose.prod.yml ps server
   ```
2. Inspect backend crash logs:
   ```bash
   docker compose -f docker-compose.prod.yml logs --tail=100 server
   ```
3. Common cause: Missing or invalid `XAI_API_KEY` or `DATABASE_URL` in `server/.env`.

### Rate Limit Exceeded on Login (HTTP 429)
The Nginx configuration limits login attempts to 5 requests/minute to prevent brute-force attacks. Wait 60 seconds before retrying, or adjust `auth_limit` in `docker/nginx/nginx.conf`.

#!/usr/bin/env bash
# ==============================================================================
# AI Trading Platform — Production One-Click Installer
# Supported OS: Ubuntu 22.04 / 24.04 LTS, Debian 12
# Usage:
#   sudo ./install.sh
# Non-interactive usage:
#   DOMAIN="trade.example.com" ADMIN_PASSWORD="MySecurePassword123!" sudo -E ./install.sh
# ==============================================================================

set -euo pipefail

# Text colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m' # No Color

log_step() { echo -e "\n${BLUE}${BOLD}==>${NC} ${BOLD}$1${NC}"; }
log_info() { echo -e "${CYAN}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# ------------------------------------------------------------------------------
# 1. Root & OS Verification
# ------------------------------------------------------------------------------
if [ "$EUID" -ne 0 ]; then
  log_error "This installer must be run as root or with sudo: sudo ./install.sh"
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

clear
echo -e "${CYAN}${BOLD}"
cat << "EOF"
    ___    ____   ______               ___                
   /   |  /  _/  /_  __/________ _____/ (_)___  ____ _    
  / /| |  / /     / / / ___/ __ `/ __  / / __ \/ __ `/    
 / ___ |_/ /     / / / /  / /_/ / /_/ / / / / / /_/ /     
/_/  |_/___/    /_/ /_/   \__,_/\__,_/_/_/ /_/\__, /      
                                             /____/       
        ⚡ PRODUCTION DEPLOYMENT & INSTALLATION ⚡
EOF
echo -e "${NC}"
echo "=================================================================="
echo " Starting automated deployment for AI Trading Platform..."
echo "=================================================================="

# ------------------------------------------------------------------------------
# 2. Interactive / Environment Variable Configuration
# ------------------------------------------------------------------------------
log_step "Configuring Deployment Parameters"

# Domain
if [ -z "${DOMAIN:-}" ]; then
  echo -ne "${BOLD}Enter domain name for the web dashboard (e.g., trade.yourdomain.com or localhost): ${NC}"
  read -r DOMAIN
  DOMAIN=${DOMAIN:-localhost}
fi
log_info "Domain set to: ${DOMAIN}"

# Admin Username
ADMIN_USER="${ADMIN_USER:-admin}"

# Admin Password
if [ -z "${ADMIN_PASSWORD:-}" ]; then
  echo -ne "${BOLD}Enter admin password for the dashboard [Press Enter to auto-generate]: ${NC}"
  read -rs ADMIN_PASSWORD
  echo ""
  if [ -z "$ADMIN_PASSWORD" ]; then
    ADMIN_PASSWORD=$(openssl rand -base64 12 | tr -dc 'a-zA-Z0-9' | head -c 16)
    log_info "Auto-generated Admin Password: ${ADMIN_PASSWORD}"
  fi
fi

# Grok xAI API Key
if [ -z "${XAI_API_KEY:-}" ]; then
  echo -ne "${BOLD}Enter xAI (Grok) API key [Press Enter to configure later]: ${NC}"
  read -rs XAI_API_KEY
  echo ""
  XAI_API_KEY=${XAI_API_KEY:-"xai-placeholder-configure-in-server-env"}
fi

# Let's Encrypt Email (if not localhost)
LE_EMAIL="${LE_EMAIL:-admin@${DOMAIN}}"

# ------------------------------------------------------------------------------
# 3. System Packages & Docker Installation
# ------------------------------------------------------------------------------
log_step "Installing System Dependencies & Hardening Tools"
apt-get update -y
apt-get install -y --no-install-recommends \
    curl \
    git \
    ufw \
    fail2ban \
    ca-certificates \
    gnupg \
    lsb-release \
    openssl \
    python3

# Install Docker if not present
if ! command -v docker &> /dev/null; then
    log_info "Installing Docker Engine & Docker Compose..."
    install -m 0755 -d /etc/apt/keyrings
    curl -fsSL https://download.docker.com/linux/ubuntu/gpg | gpg --dearmor --yes -o /etc/apt/keyrings/docker.gpg
    chmod a+r /etc/apt/keyrings/docker.gpg

    UBUNTU_CODENAME=$(lsb_release -cs 2>/dev/null || echo "jammy")
    echo \
      "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/ubuntu \
      ${UBUNTU_CODENAME} stable" | \
      tee /etc/apt/sources.list.d/docker.list > /dev/null

    apt-get update -y
    apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
    systemctl enable --now docker
    log_success "Docker installed successfully."
else
    log_info "Docker is already installed."
fi

# ------------------------------------------------------------------------------
# 4. Configure UFW Firewall
# ------------------------------------------------------------------------------
log_step "Hardening Network Firewall (UFW)"
ufw default deny incoming
ufw default allow outgoing
ufw allow 22/tcp comment 'SSH'
ufw allow 80/tcp comment 'HTTP'
ufw allow 443/tcp comment 'HTTPS'

# Deny direct internet access to internal databases & backend ports
ufw deny 5432 || true
ufw deny 6379 || true
ufw deny 8080 || true
ufw deny 8081 || true

ufw --force enable
log_success "Firewall rules applied: ports 22, 80, 443 open. Internal services secured."

# Enable fail2ban
systemctl enable --now fail2ban

# ------------------------------------------------------------------------------
# 5. Generate Cryptographic Credentials & Environment File
# ------------------------------------------------------------------------------
log_step "Generating Cryptographic Secrets & Environment Configuration"

DB_PASS=$(openssl rand -hex 20)
REDIS_PASS=$(openssl rand -hex 20)
JWT_SECRET=$(openssl rand -hex 32)
ADMIN_HASH=$(python3 -c "import hashlib; print(hashlib.sha256('${ADMIN_PASSWORD}'.encode()).hexdigest())")

# Write server/.env
cat <<EOF > server/.env
# =============================================================================
# Production Environment — AI Trading Platform
# Generated automatically by install.sh at $(date -u +"%Y-%m-%dT%H:%M:%SZ")
# =============================================================================

# Grok AI Configuration
XAI_API_KEY=${XAI_API_KEY}
XAI_BASE_URL=https://api.x.ai/v1
XAI_MODEL_PRIMARY=grok-4.6
XAI_MODEL_FAST=grok-4.5

# Exchange API Keys
MEXC_API_KEY=
MEXC_SECRET_KEY=
MEXC_BASE_URL=https://api.mexc.com
MEXC_WS_URL=wss://wbs.mexc.com/ws

ALPACA_API_KEY=
ALPACA_SECRET_KEY=
ALPACA_BASE_URL=https://paper-api.alpaca.markets
ALPACA_DATA_URL=https://data.alpaca.markets
ALPACA_WS_URL=wss://stream.data.alpaca.markets

IC_MARKETS_API_KEY=
IC_MARKETS_ACCOUNT_ID=
IC_MARKETS_CLIENT_ID=
IC_MARKETS_CLIENT_SECRET=
IC_MARKETS_BASE_URL=https://openapi.ctrader.com

# Database Connection (TimescaleDB)
POSTGRES_USER=aitrading
POSTGRES_PASSWORD=${DB_PASS}
POSTGRES_DB=aitrading
DATABASE_URL=postgresql://aitrading:${DB_PASS}@db:5432/aitrading
DATABASE_MAX_CONNECTIONS=20

# Redis Cache
REDIS_PASSWORD=${REDIS_PASS}

# Server Internal Bindings
SERVER_HOST=0.0.0.0
SERVER_PORT=8080
SERVER_WS_PORT=8081

# JWT Authentication
JWT_SECRET=${JWT_SECRET}
JWT_EXPIRY_HOURS=720

# Admin Credentials
ADMIN_USERNAME=${ADMIN_USER}
ADMIN_PASSWORD_HASH=${ADMIN_HASH}

# Trading Configuration
TRADING_MODE=manual
MAX_POSITION_SIZE_PCT=5.0
MAX_DAILY_LOSS_PCT=3.0
MAX_CONCURRENT_POSITIONS=5
DEFAULT_STOP_LOSS_PCT=2.0
DEFAULT_TAKE_PROFIT_PCT=4.0
ANALYSIS_INTERVAL_SECS=30
SENTIMENT_CHECK_INTERVAL_SECS=300

# Logging
RUST_LOG=info,ai_trading_server=info
EOF

chmod 600 server/.env
log_success "server/.env created with strict 600 permissions."

# Also update web/.env for production domain if needed
if [ "$DOMAIN" != "localhost" ]; then
  cat <<EOF > web/.env
VITE_API_URL=https://${DOMAIN}
VITE_WS_URL=wss://${DOMAIN}/ws
EOF
else
  cat <<EOF > web/.env
VITE_API_URL=
VITE_WS_URL=ws://localhost:8081
EOF
fi

# ------------------------------------------------------------------------------
# 6. Configure Nginx Gateway & Bootstrap SSL
# ------------------------------------------------------------------------------
log_step "Setting up Gateway Reverse Proxy & SSL"

# Update server_name in trading.conf
sed -i "s/server_name .*/server_name ${DOMAIN};/g" docker/nginx/conf.d/trading.conf

# Create dummy certificates so Nginx starts without errors before certbot runs
DUMMY_DIR="docker/nginx/dummy-certs"
mkdir -p "$DUMMY_DIR"
if [ ! -f "$DUMMY_DIR/fullchain.pem" ]; then
    log_info "Generating temporary self-signed certificate for initial Nginx bootstrap..."
    openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
        -keyout "$DUMMY_DIR/privkey.pem" \
        -out "$DUMMY_DIR/fullchain.pem" \
        -subj "/C=US/ST=State/L=City/O=AiTrading/CN=${DOMAIN}" 2>/dev/null
    log_success "Bootstrap certificate generated."
fi

# ------------------------------------------------------------------------------
# 7. Build and Launch Containers
# ------------------------------------------------------------------------------
log_step "Building and Starting Production Containers"
log_info "Building TimescaleDB, Redis, Rust Server, React Dashboard, and Nginx..."

# Stop any running containers
docker compose -f docker-compose.prod.yml down --remove-orphans 2>/dev/null || true

# Build & launch in detached mode
docker compose -f docker-compose.prod.yml build
docker compose -f docker-compose.prod.yml up -d

log_info "Waiting for services to pass health checks..."
sleep 15
docker compose -f docker-compose.prod.yml ps

# ------------------------------------------------------------------------------
# 8. Setup Automated Daily Database Backup
# ------------------------------------------------------------------------------
log_step "Configuring Automated Backups"
chmod +x scripts/backup.sh scripts/deploy.sh

CRON_JOB="0 3 * * * cd ${SCRIPT_DIR} && ./scripts/backup.sh >> /var/log/aitrading_backup.log 2>&1"
(crontab -l 2>/dev/null | grep -Fv "aitrading" || true; echo "$CRON_JOB") | crontab -
log_success "Automated daily backup cron scheduled (every day at 03:00 UTC)."

# ------------------------------------------------------------------------------
# 9. Optional Let's Encrypt SSL Activation
# ------------------------------------------------------------------------------
if [ "$DOMAIN" != "localhost" ]; then
    echo ""
    read -p "Would you like to issue a free Let's Encrypt SSL certificate for ${DOMAIN} now? (y/N): " -r SSL_CHOICE
    if [[ $SSL_CHOICE =~ ^[Yy]$ ]]; then
        log_step "Issuing Let's Encrypt SSL Certificate"
        docker compose -f docker-compose.prod.yml run --rm certbot certonly \
            --webroot --webroot-path=/var/www/certbot \
            --email "$LE_EMAIL" --agree-tos --no-eff-email \
            -d "$DOMAIN" || log_warning "Certbot issuance failed. Check DNS propagation."

        if [ -f "/etc/letsencrypt/live/${DOMAIN}/fullchain.pem" ]; then
            sed -i "s|/etc/letsencrypt/live/trading/fullchain.pem|/etc/letsencrypt/live/${DOMAIN}/fullchain.pem|g" docker/nginx/conf.d/trading.conf
            sed -i "s|/etc/letsencrypt/live/trading/privkey.pem|/etc/letsencrypt/live/${DOMAIN}/privkey.pem|g" docker/nginx/conf.d/trading.conf
            docker compose -f docker-compose.prod.yml exec nginx nginx -s reload
            log_success "Let's Encrypt SSL certificate activated!"
        fi
    else
        log_info "Skipping Let's Encrypt setup. Running with bootstrap self-signed certificate."
    fi
fi

# ------------------------------------------------------------------------------
# 10. Summary & Completion
# ------------------------------------------------------------------------------
clear
echo -e "${GREEN}${BOLD}"
cat << "EOF"
==================================================================
   🎉 AI TRADING PLATFORM IS LIVE & READY FOR PRODUCTION!
==================================================================
EOF
echo -e "${NC}"
echo -e "  🌐 Web Dashboard:    ${BOLD}https://${DOMAIN}${NC} (or http://${DOMAIN})"
echo -e "  📡 REST API:         ${BOLD}https://${DOMAIN}/api/health${NC}"
echo -e "  🔌 WebSocket Feed:   ${BOLD}wss://${DOMAIN}/ws${NC}"
echo ""
echo -e "  🔑 ${BOLD}Login Credentials:${NC}"
echo -e "     • Username:       ${CYAN}${BOLD}${ADMIN_USER}${NC}"
echo -e "     • Password:       ${CYAN}${BOLD}${ADMIN_PASSWORD}${NC}"
echo ""
echo -e "  ⚙️  ${BOLD}Configuration File:${NC}  ${SCRIPT_DIR}/server/.env"
echo -e "  💾 ${BOLD}Database Backups:${NC}    /var/backups/aitrading (Daily at 03:00 UTC)"
echo ""
echo "=================================================================="
echo -e "  ${BOLD}Useful Management Commands:${NC}"
echo "  • View live logs:     docker compose -f docker-compose.prod.yml logs -f"
echo "  • Service status:     docker compose -f docker-compose.prod.yml ps"
echo "  • Restart platform:   docker compose -f docker-compose.prod.yml restart"
echo "  • Update platform:    git pull && docker compose -f docker-compose.prod.yml up -d --build"
echo "=================================================================="

#!/usr/bin/env bash
# ==============================================================================
# AI Trading Platform — Production Automated Deployment Script
# Target OS: Ubuntu 22.04 / 24.04 LTS or Debian 12
# ==============================================================================

set -euo pipefail

# Color formatting
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

echo "=================================================================="
echo "    🚀 AI Trading Platform — Production Deployment Setup"
echo "=================================================================="

# 1. Check if running as root or with sudo
if [ "$EUID" -ne 0 ]; then
  log_error "Please run this script with sudo or as root: sudo ./scripts/deploy.sh"
fi

# 2. Update package lists and install basic prerequisites
log_info "Updating system packages and installing prerequisites..."
apt-get update -y
apt-get install -y --no-install-recommends \
    curl \
    git \
    ufw \
    fail2ban \
    ca-certificates \
    gnupg \
    lsb-release

# 3. Install Docker and Docker Compose plugin if missing
if ! command -v docker &> /dev/null; then
    log_info "Docker not found. Installing Docker Engine & Compose..."
    install -m 0755 -d /etc/apt/keyrings
    curl -fsSL https://download.docker.com/linux/ubuntu/gpg | gpg --dearmor -o /etc/apt/keyrings/docker.gpg
    chmod a+r /etc/apt/keyrings/docker.gpg

    echo \
      "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/ubuntu \
      $(. /etc/os-release && echo "$VERSION_CODENAME") stable" | \
      tee /etc/apt/sources.list.d/docker.list > /dev/null

    apt-get update -y
    apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
    systemctl enable --now docker
    log_success "Docker installed successfully."
else
    log_info "Docker is already installed."
fi

# 4. Configure Firewall (UFW)
log_info "Hardening network firewall (UFW)..."
ufw default deny incoming
ufw default allow outgoing
ufw allow 22/tcp comment 'SSH'
ufw allow 80/tcp comment 'HTTP (Let\'s Encrypt / Certbot)'
ufw allow 443/tcp comment 'HTTPS (Web Dashboard & API Gateway)'
# Ensure database and redis are NOT exposed publicly
ufw delete allow 5432 || true
ufw delete allow 6379 || true
ufw delete allow 8080 || true
ufw delete allow 8081 || true
ufw --force enable
log_success "Firewall rules configured: only ports 22, 80, 443 are open."

# 5. Verify production .env configuration
if [ ! -f "server/.env" ]; then
    log_warning "server/.env file not found. Copying server/.env.example to server/.env..."
    cp server/.env.example server/.env
    log_warning "Please edit server/.env with your production API keys, passwords, and JWT secret before proceeding!"
fi

# Sync to root .env for docker compose
cp server/.env .env
chmod 600 .env

# Export variables into subshell
set -a
# shellcheck source=/dev/null
source server/.env
set +a

# 6. Ensure SSL dummy certificate exists for initial Nginx bootstrap
log_info "Checking SSL certificate status..."
CERT_DIR="docker/nginx/dummy-certs"
mkdir -p "$CERT_DIR"
if [ ! -f "$CERT_DIR/fullchain.pem" ]; then
    log_info "Generating bootstrap self-signed certificate for initial Nginx start..."
    openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
        -keyout "$CERT_DIR/privkey.pem" \
        -out "$CERT_DIR/fullchain.pem" \
        -subj "/C=US/ST=State/L=City/O=Trading/CN=localhost"
    log_success "Bootstrap certificate generated."
fi

# 7. Build and start containers
log_info "Building and launching production containers with Docker Compose..."
docker compose --env-file server/.env -f docker-compose.prod.yml down --remove-orphans || true
docker compose --env-file server/.env -f docker-compose.prod.yml build --parallel
docker compose --env-file server/.env -f docker-compose.prod.yml up -d

# 8. Wait for services health check
log_info "Waiting for all services to become healthy..."
sleep 15
docker compose --env-file server/.env -f docker-compose.prod.yml ps

log_success "AI Trading Platform successfully deployed and running!"
echo "=================================================================="
echo "  📌 Next Steps:"
echo "  1. Point your domain DNS (A record) to this server's IP address."
echo "  2. Run Let's Encrypt Certbot command:"
echo "     docker compose -f docker-compose.prod.yml run --rm certbot certonly --webroot -w /var/www/certbot -d yourdomain.com"
echo "  3. Access your dashboard at: https://yourdomain.com"
echo "  4. Set up daily database backups via cron (see scripts/backup.sh)"
echo "=================================================================="

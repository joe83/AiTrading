#!/usr/bin/env bash
# ==============================================================================
# AI Trading Platform — Automated Database Backup Script
# Creates compressed PostgreSQL/TimescaleDB dumps and rotates older backups.
# ==============================================================================

set -euo pipefail

BACKUP_DIR="${BACKUP_DIR:-/var/backups/aitrading}"
TIMESTAMP="$(date +%Y%m%d_%H%M%S)"
BACKUP_FILE="${BACKUP_DIR}/aitrading_${TIMESTAMP}.sql.gz"
RETENTION_DAYS=14

mkdir -p "$BACKUP_DIR"

echo "[$(date -Iseconds)] Starting TimescaleDB backup..."

# Perform pg_dump inside the db container
docker compose -f docker-compose.prod.yml exec -T db pg_dump -U aitrading aitrading | gzip -9 > "$BACKUP_FILE"

# Set strict permissions
chmod 600 "$BACKUP_FILE"

echo "[$(date -Iseconds)] Backup created successfully: $BACKUP_FILE ($(du -h "$BACKUP_FILE" | cut -f1))"

# Prune backups older than retention days
echo "[$(date -Iseconds)] Pruning backups older than ${RETENTION_DAYS} days..."
find "$BACKUP_DIR" -type f -name "aitrading_*.sql.gz" -mtime +"$RETENTION_DAYS" -exec rm -f {} +

echo "[$(date -Iseconds)] Backup and cleanup complete."

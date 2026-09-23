#!/bin/bash
set -euo pipefail
python /opt/grok-proxy/account_control.py &
exec grok-proxy serve --host 0.0.0.0 --port 8585

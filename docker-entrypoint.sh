#!/bin/bash
# aiapp-mb docker entrypoint: auto-generate secrets + ensure MoonBit toolchain
set -e

# Auto-generate AUTH_SECRET if not set
if [ -z "$AUTH_SECRET" ] || [ "$AUTH_SECRET" = "change-me-to-a-long-random-secret-at-least-32-bytes" ]; then
  AUTH_SECRET=$(openssl rand -hex 32)
  echo "[entrypoint] AUTH_SECRET auto-generated"
fi
export AUTH_SECRET

# Auto-generate ADMIN_PASSWORD if not set or default
if [ -z "$ADMIN_PASSWORD" ] || [ "$ADMIN_PASSWORD" = "change-me-admin-password" ] || [ "$ADMIN_PASSWORD" = "admin123" ]; then
  ADMIN_PASSWORD=$(openssl rand -hex 12)
  echo "[entrypoint] ADMIN_PASSWORD auto-generated: $ADMIN_PASSWORD"
fi
export ADMIN_PASSWORD

# Ensure MoonBit toolchain is available (auto-install if missing)
if ! command -v moon &>/dev/null; then
  echo "[entrypoint] MoonBit toolchain not found, installing..."
  curl -fsSL https://cli.moonbitlang.com/install/unix.sh | bash
  export PATH="$HOME/.moon/bin:$PATH"
fi

# Ensure MOON_HOME is set (for toolchain cache, persisted on volume)
if [ -z "$MOON_HOME" ]; then
  export MOON_HOME=/data/moon
fi

exec /app/bin/aiapp-web "$@"
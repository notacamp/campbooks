#!/bin/sh
# Starts laya-serve, refusing to run without an API key: laya-serve accepts
# unauthenticated requests when LAYA_API_KEY is empty (ADR 0009).
set -eu

if [ -z "${LAYA_API_KEY:-}" ]; then
  echo "laya: LAYA_API_KEY is empty. Set CAMPBOOKS_AI__DECIDE__API_KEY in .env" >&2
  echo "laya: (for example: openssl rand -hex 32) and run docker compose up again." >&2
  exit 64
fi

exec laya-serve "$@"

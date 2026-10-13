#!/bin/sh
# Registers Campbooks' scopes and first-party OIDC clients in the identity
# provider (Rauthy) through its admin API. Runs after `idp` is healthy on every
# `docker compose up`, and is idempotent:
#
# - a missing scope is created; existing scopes are left alone;
# - a client is created and configured once, then marked done. Later runs never
#   touch it again, so changes made in the admin UI survive. To re-apply the
#   defaults, delete the client in the admin UI and remove its marker:
#     docker compose run --rm idp-init rm /app/data/campbooks/provisioned/<client-id>
#
# Rauthy's own first-boot bootstrap files cannot set a client's audience
# (`default_aud`), which is why this uses the API.
set -eu
umask 077

IDP="${IDP_INTERNAL_URL:-http://idp:8080}/auth/v1"
PUBLIC_URL="${CAMPBOOKS_PUBLIC_URL:?CAMPBOOKS_PUBLIC_URL must be set}"
PUBLIC_URL="${PUBLIC_URL%/}"
# Where the IdP delivers back-channel logouts to the server's BFF. Rauthy only
# calls https URLs, so the default is the public URL when it is https, and
# none (sessions then end on their own timeouts) when it is plain http.
case "$PUBLIC_URL" in
  https://*) BACKCHANNEL_LOGOUT_URL="${CAMPBOOKS_BACKCHANNEL_LOGOUT_URL:-$PUBLIC_URL/bff/backchannel-logout}" ;;
  *) BACKCHANNEL_LOGOUT_URL="${CAMPBOOKS_BACKCHANNEL_LOGOUT_URL:-}" ;;
esac
STATE=/app/data/campbooks
# The `server-secrets` volume (see deploy/server/init.sh): the server reads the
# web client's secret from here.
SECRETS=/secrets
API_KEY="$(cat "$STATE/api-key")"
BODY="$(mktemp)"

# Scopes Campbooks clients may request. `openid`, `profile` and `email` are
# built into Rauthy. `offline_access` is accepted for client compatibility;
# Rauthy issues refresh tokens to clients with the refresh_token flow enabled.
# The rest mirror `campbooks-server scopes`.
SCOPES="offline_access system:read workspaces:read workspaces:write workspaces:admin"
# The API's audience: every first-party token carries it in `aud`.
AUDIENCE="campbooks-api"

# api <method> <path> [json] -> prints the HTTP status, body in $BODY
api() {
  if [ $# -ge 3 ]; then
    curl -sS -o "$BODY" -w '%{http_code}' -X "$1" "$IDP$2" \
      -H "Authorization: API-Key $API_KEY" -H 'Content-Type: application/json' \
      --data "$3"
  else
    curl -sS -o "$BODY" -w '%{http_code}' -X "$1" "$IDP$2" \
      -H "Authorization: API-Key $API_KEY"
  fi
}

fail() {
  echo "idp-provision: $1 (HTTP $2): $(cat "$BODY")" >&2
  exit 1
}

ensure_scopes() {
  status="$(api GET /scopes)"
  [ "$status" = 200 ] || fail "cannot list scopes" "$status"
  for scope in $SCOPES; do
    if grep -q "\"name\":\"$scope\"" "$BODY"; then
      continue
    fi
    status="$(api POST /scopes "{\"scope\":\"$scope\"}")"
    [ "$status" = 200 ] || fail "cannot create scope $scope" "$status"
    echo "idp-provision: created scope $scope"
    api GET /scopes >/dev/null
  done
}

# ensure_client <id> <confidential> <update-json>
ensure_client() {
  id="$1"
  if [ -f "$STATE/provisioned/$id" ]; then
    echo "idp-provision: client $id already provisioned"
    return
  fi
  status="$(api GET "/clients/$id")"
  if [ "$status" = 404 ]; then
    status="$(api POST /clients "{\"id\":\"$id\",\"confidential\":$2,\"redirect_uris\":[]}")"
    [ "$status" = 200 ] || fail "cannot create client $id" "$status"
    echo "idp-provision: created client $id"
  elif [ "$status" != 200 ]; then
    fail "cannot read client $id" "$status"
  fi
  status="$(api PUT "/clients/$id" "$3")"
  [ "$status" = 200 ] || fail "cannot configure client $id" "$status"
  mkdir -p "$STATE/provisioned"
  date -u +%Y-%m-%dT%H:%M:%SZ >"$STATE/provisioned/$id"
  echo "idp-provision: configured client $id"
}

# Saves a confidential client's secret where the Campbooks server can read it
# (group-readable: the server runs with supplementary group 10001).
save_secret() {
  id="$1"
  file="$SECRETS/$id.client-secret"
  [ -s "$file" ] && return
  status="$(api POST "/clients/$id/secret")"
  [ "$status" = 200 ] || fail "cannot read the secret of client $id" "$status"
  sed -n 's/.*"secret":"\([^"]*\)".*/\1/p' "$BODY" >"$file.tmp"
  [ -s "$file.tmp" ] || fail "no secret in the response for client $id" "$status"
  chmod 0440 "$file.tmp"
  mv "$file.tmp" "$file"
  echo "idp-provision: saved the secret of client $id"
}

ensure_scopes

# Web app: the server's BFF runs the authorization-code flow (ADR 0004).
if [ -n "$BACKCHANNEL_LOGOUT_URL" ]; then
  backchannel="\"backchannel_logout_uri\": \"$BACKCHANNEL_LOGOUT_URL\","
else
  backchannel=""
fi
ensure_client campbooks-web true "$(cat <<EOF
{
  "name": "Campbooks Web",
  "confidential": true,
  "enabled": true,
  "redirect_uris": ["$PUBLIC_URL/bff/callback"],
  "post_logout_redirect_uris": ["$PUBLIC_URL/"],
  $backchannel
  "flows_enabled": ["authorization_code", "refresh_token"],
  "challenges": ["S256"],
  "access_token_alg": "EdDSA",
  "id_token_alg": "EdDSA",
  "auth_code_lifetime": 60,
  "access_token_lifetime": 900,
  "scopes": ["openid", "profile", "email", "offline_access", "system:read",
             "workspaces:read", "workspaces:write", "workspaces:admin"],
  "default_scopes": ["openid"],
  "default_aud": ["$AUDIENCE"],
  "force_mfa": false,
  "claims_at_root": false
}
EOF
)"
save_secret campbooks-web

# Desktop and mobile apps (Tauri): public client, PKCE in the system browser,
# loopback on any port or the app's own URL scheme.
ensure_client campbooks-native false "$(cat <<EOF
{
  "name": "Campbooks App",
  "confidential": false,
  "enabled": true,
  "redirect_uris": ["http://127.0.0.1/callback", "campbooks://callback"],
  "flows_enabled": ["authorization_code", "refresh_token"],
  "challenges": ["S256"],
  "access_token_alg": "EdDSA",
  "id_token_alg": "EdDSA",
  "auth_code_lifetime": 60,
  "access_token_lifetime": 900,
  "scopes": ["openid", "profile", "email", "offline_access", "system:read",
             "workspaces:read", "workspaces:write", "workspaces:admin"],
  "default_scopes": ["openid"],
  "default_aud": ["$AUDIENCE"],
  "force_mfa": false,
  "claims_at_root": false
}
EOF
)"

rm -f "$BODY"
echo "idp-provision: done"

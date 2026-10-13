#!/bin/sh
# Registers Campbooks' scopes, first-party OIDC clients and sign-in providers
# (Google, Microsoft, Zoho) in the identity provider (Rauthy) through its admin
# API. Runs after `idp` is healthy on every `docker compose up`, and is
# idempotent:
#
# - a missing scope is created; existing scopes are left alone;
# - a client is created and configured once, then marked done. Later runs never
#   touch it again, so changes made in the admin UI survive. To re-apply the
#   defaults, delete the client in the admin UI and remove its marker:
#     docker compose run --rm idp-init rm /app/data/campbooks/provisioned/<client-id>
# - a sign-in provider is registered only when its client ID is set, created
#   once and marked done with a fingerprint of its settings. It is configured
#   again only when those settings (client ID, secret file, tenant or issuer,
#   whether it creates accounts) change, so admin UI edits survive otherwise.
#   See "Sign in with Google, Microsoft or Zoho" in deploy/README.md.
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
# The `idp-data` volume (overridable for tests).
STATE="${IDP_DATA_DIR:-/app/data}/campbooks"
# The `server-secrets` volume (see deploy/server/init.sh): the server reads the
# web client's secret from here.
SECRETS="${SERVER_SECRETS_DIR:-/secrets}"
API_KEY="$(cat "$STATE/api-key")"
BODY="$(mktemp)"
# Request bodies that carry a secret: a 0600 file, never the command line.
REQUEST="$(mktemp)"
trap 'rm -f "$BODY" "$REQUEST"' EXIT

# Scopes Campbooks clients may request. `openid`, `profile` and `email` are
# built into Rauthy. `offline_access` is accepted for client compatibility;
# Rauthy issues refresh tokens to clients with the refresh_token flow enabled.
# The rest mirror `campbooks-server scopes`.
SCOPES="offline_access system:read workspaces:read workspaces:write workspaces:admin"
# The API's audience: every first-party token carries it in `aud`.
AUDIENCE="campbooks-api"

# api_file <method> <path> <file> -> like api, with the JSON body read from a file
api_file() {
  curl -sS -o "$BODY" -w '%{http_code}' -X "$1" "$IDP$2" \
    -H "Authorization: API-Key $API_KEY" -H 'Content-Type: application/json' \
    --data-binary "@$3"
}

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

# --- Sign-in providers (Google, Microsoft, Zoho) ----------------------------------
# Each one is registered only when its client ID is set. Its client secret is
# read from a file (compose mounts deploy/secrets/ read-only), never from the
# environment, and never printed: Rauthy's provider responses echo it.

die() {
  echo "idp-provision: $1" >&2
  exit 1
}

# The page each provider sends people back to; register it in its console.
IDP_PUBLIC_URL="${CAMPBOOKS_IDP_URL:-http://localhost:8081}"
IDP_PUBLIC_URL="${IDP_PUBLIC_URL%/}"
CALLBACK_URL="$IDP_PUBLIC_URL/auth/v1/providers/callback"
PROVIDER_SCOPE="openid email profile"
SECRET_DIR=/run/secrets/campbooks

# Set by provider_onboarding: may a provider create an account for someone who
# has none? It follows Campbooks' signup policy unless
# CAMPBOOKS_IDP_PROVIDER_SIGNUP overrides it.
ONBOARDING=""
SIGNUP_POLICY="${CAMPBOOKS_SIGNUP__POLICY:-open}"
PROVIDER_SIGNUP="${CAMPBOOKS_IDP_PROVIDER_SIGNUP:-auto}"
provider_onboarding() {
  case "$PROVIDER_SIGNUP" in
    open) ONBOARDING=true ;;
    closed) ONBOARDING=false ;;
    auto)
      case "$SIGNUP_POLICY" in
        # Strangers may ask for a workspace under these policies, so they must
        # be able to sign in; Campbooks applies the policy when they try.
        open | code | approval) ONBOARDING=true ;;
        # Only people who already have an account.
        invite_only) ONBOARDING=false ;;
        *) die "unknown CAMPBOOKS_SIGNUP__POLICY '$SIGNUP_POLICY' (open, invite_only, approval or code)" ;;
      esac
      ;;
    *) die "unknown CAMPBOOKS_IDP_PROVIDER_SIGNUP '$PROVIDER_SIGNUP' (auto, open or closed)" ;;
  esac
}

# Set by provider_secret: the client secret, as read and JSON-escaped.
SECRET=""
SECRET_JSON=""
# provider_secret <key> <file>: reads and checks a client secret file.
provider_secret() {
  [ -f "$2" ] || die "$1: no client secret file at $2 (see deploy/README.md)"
  [ -r "$2" ] || die "$1: cannot read $2; make it readable by uid 10001 (see deploy/README.md)"
  SECRET="$(tr -d '\r\n' <"$2")"
  [ -n "$SECRET" ] || die "$1: the client secret file $2 is empty"
  case "$SECRET" in
    *[![:print:]]*) die "$1: the client secret file $2 contains control characters" ;;
  esac
  SECRET_JSON="$(printf '%s' "$SECRET" | sed 's/\\/\\\\/g; s/"/\\"/g')"
}

# provider_fail <key> <message> <status>: like fail, but withholds a response
# that contains the secret.
provider_fail() {
  if grep -qF -e "$SECRET" -e "$SECRET_JSON" "$BODY"; then
    echo "idp-provision: $1: $2 (HTTP $3; response withheld: it contains the client secret)" >&2
  else
    echo "idp-provision: $1: $2 (HTTP $3): $(cat "$BODY")" >&2
  fi
  exit 1
}

# provider_denied <status>: true, and remembered, when the API key may not
# manage providers (installs from before sign-in providers existed).
NO_PROVIDER_RIGHTS=""
provider_denied() {
  case "$1" in
    401 | 403) NO_PROVIDER_RIGHTS=1 ;;
    *) return 1 ;;
  esac
}

# mark_provider <key> <id> <fingerprint>
mark_provider() {
  mkdir -p "$STATE/provisioned"
  printf 'id=%s\nfingerprint=%s\n' "$2" "$3" >"$STATE/provisioned/provider-$1.tmp"
  mv "$STATE/provisioned/provider-$1.tmp" "$STATE/provisioned/provider-$1"
}

# ensure_provider <key> <name> <typ> <issuer> <authorize> <token> <userinfo>
#                 <jwks> <client-id> <secret-file>
ensure_provider() {
  key="$1" issuer="$4" client_id="$9" secret_file="${10}"
  case "$client_id" in
    *[!A-Za-z0-9._~@:/+-]*) die "$key: the client ID contains unexpected characters" ;;
  esac
  provider_secret "$key" "$secret_file"

  cat >"$REQUEST" <<JSON
{
  "name": "$2",
  "typ": "$3",
  "enabled": true,
  "issuer": "$issuer",
  "authorization_endpoint": "$5",
  "token_endpoint": "$6",
  "userinfo_endpoint": "$7",
  "jwks_endpoint": "$8",
  "use_pkce": true,
  "client_secret_basic": false,
  "client_secret_post": true,
  "auto_onboarding": $ONBOARDING,
  "auto_link": false,
  "client_id": "$client_id",
  "client_secret": "$SECRET_JSON",
  "scope": "$PROVIDER_SCOPE"
}
JSON
  # The operator's inputs, not the script's defaults: a Campbooks upgrade never
  # rewrites a provider by itself.
  fingerprint="$(printf '%s\n' "$client_id" "$SECRET" "$issuer" "$ONBOARDING" | sha256sum | cut -d' ' -f1)"
  marker="$STATE/provisioned/provider-$key"

  if [ -f "$marker" ]; then
    id="$(sed -n 's/^id=//p' "$marker")"
    if [ "$(sed -n 's/^fingerprint=//p' "$marker")" = "$fingerprint" ]; then
      echo "idp-provision: sign-in provider $key already provisioned"
      return
    fi
    status="$(api_file PUT "/providers/$id" "$REQUEST")"
    provider_denied "$status" && return
    case "$status" in
      200)
        mark_provider "$key" "$id" "$fingerprint"
        echo "idp-provision: re-applied sign-in provider $key because its settings changed (edits made to it in the admin UI were replaced)"
        return
        ;;
      404) ;; # deleted in the admin UI: create it again below
      *) provider_fail "$key" "cannot update sign-in provider" "$status" ;;
    esac
  else
    # Adopt a provider with the same issuer (added by hand, or a lost marker)
    # instead of creating a second one. Rauthy lists one flat JSON object per
    # provider, `id` first.
    status="$(api POST /providers)"
    provider_denied "$status" && return
    [ "$status" = 200 ] || provider_fail "$key" "cannot list sign-in providers" "$status"
    id="$(tr '{' '\n' <"$BODY" | grep -F "\"issuer\":\"$issuer\"" |
      sed -n 's/^"id":"\([^"]*\)".*/\1/p' | head -n 1)"
    if [ -n "$id" ]; then
      status="$(api_file PUT "/providers/$id" "$REQUEST")"
      [ "$status" = 200 ] || provider_fail "$key" "cannot configure sign-in provider" "$status"
      mark_provider "$key" "$id" "$fingerprint"
      echo "idp-provision: configured existing sign-in provider $key"
      return
    fi
  fi

  status="$(api_file POST /providers/create "$REQUEST")"
  provider_denied "$status" && return
  [ "$status" = 200 ] || provider_fail "$key" "cannot create sign-in provider" "$status"
  id="$(sed -n 's/^{"id":"\([^"]*\)".*/\1/p' "$BODY")"
  [ -n "$id" ] || provider_fail "$key" "no provider id in the response" "$status"
  mark_provider "$key" "$id" "$fingerprint"
  echo "idp-provision: created sign-in provider $key"
}

# A provider removed from .env stays in the IdP: deleting it would strand the
# accounts linked to it.
provider_unset() {
  if [ -f "$STATE/provisioned/provider-$1" ]; then
    echo "idp-provision: sign-in provider $1 is no longer configured but stays registered; disable or delete it in the admin UI (Providers)"
  fi
}

if [ -n "${CAMPBOOKS_IDP_GOOGLE_CLIENT_ID:-}${CAMPBOOKS_IDP_MICROSOFT_CLIENT_ID:-}${CAMPBOOKS_IDP_ZOHO_CLIENT_ID:-}" ]; then
  provider_onboarding
fi

if [ -n "${CAMPBOOKS_IDP_GOOGLE_CLIENT_ID:-}" ]; then
  ensure_provider google Google google https://accounts.google.com \
    https://accounts.google.com/o/oauth2/v2/auth \
    https://oauth2.googleapis.com/token \
    https://openidconnect.googleapis.com/v1/userinfo \
    https://www.googleapis.com/oauth2/v3/certs \
    "$CAMPBOOKS_IDP_GOOGLE_CLIENT_ID" \
    "${CAMPBOOKS_IDP_GOOGLE_CLIENT_SECRET_FILE:-$SECRET_DIR/google-client-secret}"
else
  provider_unset google
fi

if [ -n "${CAMPBOOKS_IDP_MICROSOFT_CLIENT_ID:-}" ]; then
  # common: work, school and personal accounts; organizations: work and school;
  # consumers: personal only; or one organization's tenant ID or domain.
  tenant="${CAMPBOOKS_IDP_MICROSOFT_TENANT:-common}"
  case "$tenant" in
    "" | *[!A-Za-z0-9.-]*) die "microsoft: CAMPBOOKS_IDP_MICROSOFT_TENANT must be common, organizations, consumers, a tenant ID or a domain" ;;
  esac
  ms="https://login.microsoftonline.com/$tenant"
  ensure_provider microsoft Microsoft oidc "$ms/v2.0" \
    "$ms/oauth2/v2.0/authorize" \
    "$ms/oauth2/v2.0/token" \
    https://graph.microsoft.com/oidc/userinfo \
    "$ms/discovery/v2.0/keys" \
    "$CAMPBOOKS_IDP_MICROSOFT_CLIENT_ID" \
    "${CAMPBOOKS_IDP_MICROSOFT_CLIENT_SECRET_FILE:-$SECRET_DIR/microsoft-client-secret}"
else
  provider_unset microsoft
fi

if [ -n "${CAMPBOOKS_IDP_ZOHO_CLIENT_ID:-}" ]; then
  # Zoho runs one accounts server per data centre (accounts.zoho.eu,
  # accounts.zoho.com, accounts.zoho.in, ...); people whose Zoho account lives
  # in another one cannot sign in through this provider.
  zoho="${CAMPBOOKS_IDP_ZOHO_ISSUER:-https://accounts.zoho.eu}"
  zoho="${zoho%/}"
  case "$zoho" in
    https://*) ;;
    *) die "zoho: CAMPBOOKS_IDP_ZOHO_ISSUER must be an https:// URL, e.g. https://accounts.zoho.eu" ;;
  esac
  case "${zoho#https://}" in
    "" | *[!A-Za-z0-9.-]*) die "zoho: CAMPBOOKS_IDP_ZOHO_ISSUER must be the accounts server, without a path" ;;
  esac
  ensure_provider zoho Zoho oidc "$zoho" \
    "$zoho/oauth/v2/auth" \
    "$zoho/oauth/v2/token" \
    "$zoho/oauth/v2/userinfo" \
    "$zoho/oauth/v2/keys" \
    "$CAMPBOOKS_IDP_ZOHO_CLIENT_ID" \
    "${CAMPBOOKS_IDP_ZOHO_CLIENT_SECRET_FILE:-$SECRET_DIR/zoho-client-secret}"
else
  provider_unset zoho
fi

if [ -n "$NO_PROVIDER_RIGHTS" ]; then
  echo "idp-provision: the IdP refused to manage sign-in providers: its \`campbooks\` API key predates them." >&2
  echo "idp-provision: Grant it once in the admin UI ($IDP_PUBLIC_URL/auth/v1/admin): API Keys -> campbooks ->" >&2
  echo "idp-provision: Auth Providers: read, create, update. Then run \`docker compose up -d\` again." >&2
  exit 1
fi
if [ -n "$ONBOARDING" ]; then
  echo "idp-provision: sign-in providers may create accounts for new people: $ONBOARDING (CAMPBOOKS_SIGNUP__POLICY=$SIGNUP_POLICY, CAMPBOOKS_IDP_PROVIDER_SIGNUP=$PROVIDER_SIGNUP)"
  echo "idp-provision: redirect URI to register with each provider: $CALLBACK_URL"
fi


echo "idp-provision: done"

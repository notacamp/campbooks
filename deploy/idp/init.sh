#!/bin/sh
# Prepares the identity provider (Rauthy). Runs as a one-shot container before
# `idp` on every `docker compose up`.
#
# Once, on the very first run, it generates every secret into the `idp-data`
# volume and never overwrites them afterwards:
#   secrets.toml                     encryption key + cluster secrets
#   bootstrap/api_keys.json          the `campbooks` admin API key (first boot):
#                                    clients, scopes, client secrets and
#                                    sign-in providers
#   campbooks/api-key                that key's token, used by provision.sh
#   campbooks/initial-admin-password the first admin password
#
# On every run it rewrites config.toml from the environment (so changing
# CAMPBOOKS_IDP_URL and restarting is enough) and refreshes the static busybox
# used by the `idp` healthcheck (Rauthy's image is distroless).
set -eu
umask 077

DATA=/app/data
STATE="$DATA/campbooks"
IDP_URL="${CAMPBOOKS_IDP_URL:-http://localhost:8081}"
IDP_URL="${IDP_URL%/}"
ADMIN_EMAIL="${CAMPBOOKS_IDP_ADMIN_EMAIL:-admin@localhost}"
RAUTHY_UID=10001

mkdir -p "$STATE" "$DATA/bin"

# rand <charset> <length>
rand() { tr -dc "$1" </dev/urandom | head -c "$2"; }

if [ ! -f "$STATE/initialized" ]; then
  key_id="$(rand 'a-z0-9' 8)"
  enc_key="$(head -c 32 /dev/urandom | base64)"
  api_key_secret="$(rand 'A-Za-z0-9' 64)"
  # Upper, lower and digits are always present, whatever the password policy.
  admin_password="$(rand 'A-Z' 4)$(rand 'a-z' 8)$(rand '0-9' 4)$(rand 'A-Za-z0-9' 12)"

  tmp="$(mktemp -d "$DATA/.init.XXXXXX")"
  mkdir -p "$tmp/bootstrap"

  cat >"$tmp/secrets.toml" <<EOF
# Generated once by deploy/idp/init.sh. Back this file up with the volume:
# without the encryption key, the IdP's encrypted data cannot be read.
[cluster]
secret_raft = '$(rand 'A-Za-z0-9' 48)'
secret_api = '$(rand 'A-Za-z0-9' 48)'

[encryption]
keys = ['$key_id/$enc_key']
key_active = '$key_id'
EOF

  # Read by Rauthy only while it initializes an empty database.
  cat >"$tmp/bootstrap/api_keys.json" <<EOF
[
  {
    "name": "campbooks",
    "secret": { "Plain": "$api_key_secret" },
    "access": [
      { "group": "Clients", "access_rights": ["read", "create", "update", "delete"] },
      { "group": "Scopes", "access_rights": ["read", "create", "update", "delete"] },
      { "group": "Secrets", "access_rights": ["read"] },
      { "group": "AuthProviders", "access_rights": ["read", "create", "update"] }
    ]
  }
]
EOF
  printf '%s' "campbooks\$$api_key_secret" >"$tmp/api-key"
  printf '%s\n' "$admin_password" >"$tmp/initial-admin-password"

  mv "$tmp/secrets.toml" "$DATA/secrets.toml"
  rm -rf "$DATA/bootstrap"
  mv "$tmp/bootstrap" "$DATA/bootstrap"
  mv "$tmp/api-key" "$STATE/api-key"
  mv "$tmp/initial-admin-password" "$STATE/initial-admin-password"
  rmdir "$tmp"
  date -u +%Y-%m-%dT%H:%M:%SZ >"$STATE/initialized"
  echo "idp-init: generated the IdP secrets"
  echo "idp-init: first admin login is $ADMIN_EMAIL; read its password with"
  echo "idp-init:   docker compose run --rm idp-init cat $STATE/initial-admin-password"
fi

# --- config.toml, from the environment -------------------------------------------
case "$IDP_URL" in
  https://*) scheme=https proxy_mode=true default_port=443 ;;
  http://*) scheme=http proxy_mode=false default_port=80 ;;
  *) echo "idp-init: CAMPBOOKS_IDP_URL must start with http:// or https://" >&2; exit 1 ;;
esac
host_port="${IDP_URL#*://}"
case "$host_port" in
  */*) echo "idp-init: CAMPBOOKS_IDP_URL must not have a path: $IDP_URL" >&2; exit 1 ;;
esac
host="${host_port%%:*}"
if [ "$host" = "$host_port" ]; then port="$default_port"; else port="${host_port##*:}"; fi

# Behind a TLS-terminating proxy, Rauthy trusts forwarded headers only from
# these networks (default: private ranges, which covers Docker networks).
# Loopback is always trusted so the container's own healthcheck gets through.
proxies="'127.0.0.1/32', "
for cidr in ${CAMPBOOKS_IDP_TRUSTED_PROXIES:-10.0.0.0/8 172.16.0.0/12 192.168.0.0/16}; do
  proxies="$proxies'$cidr', "
done

cat >"$DATA/config.toml.tmp" <<EOF
# Rewritten by deploy/idp/init.sh on every start. Do not edit; set the
# CAMPBOOKS_IDP_* variables in .env instead. Secrets live in secrets.toml.

[bootstrap]
admin_email = '$ADMIN_EMAIL'
password_plain = '$(cat "$STATE/initial-admin-password")'
bootstrap_dir = '$DATA/bootstrap'

[cluster]
node_id = 1
nodes = ['1 localhost:8100 localhost:8200']
secret_raft = '\$SECRETS'
secret_api = '\$SECRETS'

[encryption]
keys = ['\$SECRETS']
key_active = '\$SECRETS'

[access]
# RFC 8252: loopback redirects (native apps) match on any port. Rauthy >= 0.37
# applies this to every client; 0.36 only to dynamic and ephemeral ones.
rfc_8252_enable = true

[server]
scheme = 'http'
port_http = 8080
pub_url = '$host_port'
proxy_mode = $proxy_mode
trusted_proxies = [$proxies]

[webauthn]
rp_id = '$host'
rp_origin = '$scheme://$host:$port'
rp_name = 'Campbooks'
EOF
mv "$DATA/config.toml.tmp" "$DATA/config.toml"

cp /bin/busybox "$DATA/bin/busybox.tmp"
chmod 0755 "$DATA/bin/busybox.tmp"
mv "$DATA/bin/busybox.tmp" "$DATA/bin/busybox"

chown -R "$RAUTHY_UID:$RAUTHY_UID" "$DATA"
echo "idp-init: issuer will be $IDP_URL/auth/v1/"

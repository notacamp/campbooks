#!/bin/sh
# Prepares the `server-secrets` volume. Runs as a one-shot container before
# `server` (and `idp-provision`) on every `docker compose up`.
#
# Once, on the very first run, it generates:
#   secret-key   CAMPBOOKS_SECRET_KEY: 32 random bytes, base64. The server
#                encrypts the tokens of web sessions with a key derived from
#                it. Replacing it signs every web user out (nothing else is
#                lost); see deploy/README.md.
#
# `idp-provision` later writes `campbooks-web.client-secret` here (the web
# app's OIDC client secret). The server mounts the volume read-only and reads
# both through its supplementary group 10001 (the files are 0440).
set -eu
umask 077

DIR=/secrets
OWNER=10001

mkdir -p "$DIR"
if [ ! -s "$DIR/secret-key" ]; then
  head -c 32 /dev/urandom | base64 >"$DIR/secret-key.tmp"
  mv "$DIR/secret-key.tmp" "$DIR/secret-key"
  echo "server-init: generated the instance secret key"
fi

# 0750 directory owned by the IdP's user, so idp-provision (uid 10001) can
# write the client secret; group-readable files for the server.
chown "$OWNER:$OWNER" "$DIR" "$DIR/secret-key"
chmod 0750 "$DIR"
chmod 0440 "$DIR/secret-key"

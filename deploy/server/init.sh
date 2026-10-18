#!/bin/sh
# Prepares the `server-secrets` volume. Runs as a one-shot container before
# `server` on every `docker compose up`.
#
# Once, on the very first run, it generates:
#   secret-key   CAMPBOOKS_SECRET_KEY: 32 random bytes, base64. The server
#                encrypts the tokens of web sessions with a key derived from
#                it. Replacing it signs every web user out (nothing else is
#                lost); see deploy/README.md.
#
# `init.sh rotate` (run by hand, see deploy/README.md "Rotating the instance
# secret key") moves the key to `secret-key-previous` and generates a new
# one; `init.sh forget-previous` deletes `secret-key-previous` once the
# rotation is over. Neither prints a key.
#
# The server mounts the volume read-only and reads the keys through its
# supplementary group 10001 (the files are 0440).
set -eu
umask 077

DIR=/secrets
OWNER=10001

mkdir -p "$DIR"
case "${1:-}" in
  rotate)
    if [ -s "$DIR/secret-key-previous" ]; then
      echo "server-init: a rotation is already under way (secret-key-previous exists);" \
        "finish it with forget-previous first" >&2
      exit 1
    fi
    if [ ! -s "$DIR/secret-key" ]; then
      echo "server-init: there is no secret key to rotate" >&2
      exit 1
    fi
    mv "$DIR/secret-key" "$DIR/secret-key-previous"
    echo "server-init: moved the instance secret key to secret-key-previous"
    ;;
  forget-previous)
    rm -f "$DIR/secret-key-previous"
    echo "server-init: deleted secret-key-previous"
    ;;
  "") ;;
  *)
    echo "server-init: unknown command: $1 (rotate or forget-previous)" >&2
    exit 1
    ;;
esac
if [ ! -s "$DIR/secret-key" ]; then
  head -c 32 /dev/urandom | base64 >"$DIR/secret-key.tmp"
  mv "$DIR/secret-key.tmp" "$DIR/secret-key"
  echo "server-init: generated the instance secret key"
fi

# 0750 directory and group-readable keys for the server.
chown "$OWNER:$OWNER" "$DIR" "$DIR/secret-key"
chmod 0750 "$DIR"
chmod 0440 "$DIR/secret-key"
if [ -s "$DIR/secret-key-previous" ]; then
  chown "$OWNER:$OWNER" "$DIR/secret-key-previous"
  chmod 0440 "$DIR/secret-key-previous"
fi

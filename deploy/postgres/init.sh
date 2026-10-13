#!/bin/sh
# Creates the database login the server queries as. Runs as a one-shot
# container (`db-init`) before `server` on every `docker compose up`.
#
# Why: the server must not run queries as the owner/superuser login, because
# Postgres skips row-level security for it and a SQL injection could switch
# role. So the server connects as `campbooks_runtime` instead: a plain login
# (no superuser, no BYPASSRLS, owns nothing) that is a member of the runtime
# role `campbooks_app`, which the migrations grant the tables to. The owner
# login (POSTGRES_USER) is used only to apply migrations.
#
# Once, on the first run, it generates the login's password into the
# `db-secrets` volume and never overwrites it afterwards. On every run it
# (re)applies the role, its attributes and its password, so it is safe to
# repeat and upgrades an existing installation with no manual step. The
# server reads the password from the file (CAMPBOOKS_DATABASE_PASSWORD_FILE);
# it is never put in the environment or in .env.
#
# Needs PGHOST, PGUSER, PGPASSWORD and PGDATABASE (the owner login) in the
# environment.
set -eu
umask 077

DIR=/run/db
FILE="$DIR/runtime-password"
# `nonroot` in the server image.
SERVER_UID=65532

mkdir -p "$DIR"
if [ ! -s "$FILE" ]; then
  tmp="$(mktemp "$DIR/.password.XXXXXX")"
  tr -dc 'A-Za-z0-9' </dev/urandom | head -c 48 >"$tmp"
  mv "$tmp" "$FILE"
fi
chown "$SERVER_UID:$SERVER_UID" "$FILE"
chmod 0400 "$FILE"

# The password goes in as a psql variable, quoted by psql itself.
psql -v ON_ERROR_STOP=1 -v "pw=$(cat "$FILE")" <<'SQL'
-- The runtime role (also created by the first migration; both tolerate the other).
SELECT 'CREATE ROLE campbooks_app NOLOGIN NOSUPERUSER NOBYPASSRLS NOCREATEDB NOCREATEROLE'
 WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'campbooks_app') \gexec
SELECT 'CREATE ROLE campbooks_runtime LOGIN'
 WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'campbooks_runtime') \gexec
ALTER ROLE campbooks_runtime WITH LOGIN NOSUPERUSER NOBYPASSRLS NOCREATEDB NOCREATEROLE
  NOREPLICATION INHERIT PASSWORD :'pw';
GRANT campbooks_app TO campbooks_runtime;
SQL
echo "db-init: runtime login ready"

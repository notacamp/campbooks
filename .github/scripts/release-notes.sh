#!/usr/bin/env bash
# Print the CHANGELOG.md section for a release of next, e.g.
#   .github/scripts/release-notes.sh 0.2.0   # → body of "## [next-v0.2.0] - <date>"
# Exits 2 when the section is missing or empty, so callers can tell "nothing to
# release yet" from a real failure. Used by release-next.yml and the policy gate.
set -euo pipefail

version="${1:?usage: release-notes.sh <semver>}"
changelog="${2:-CHANGELOG.md}"

if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "release-notes: '$version' is not X.Y.Z" >&2
  exit 1
fi

notes=$(awk -v tag="## [next-v${version}]" '
  index($0, tag) == 1 { on = 1; next }
  /^## \[/ { on = 0 }
  on
' "$changelog")

# Trim blank lines at both ends.
notes=$(printf '%s\n' "$notes" | sed -e :a -e '/^[[:space:]]*$/{$d;N;ba' -e '}' | sed '/./,$!d')

if [ -z "$notes" ]; then
  echo "release-notes: no non-empty '## [next-v${version}]' section in $changelog" >&2
  exit 2
fi
printf '%s\n' "$notes"

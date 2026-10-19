#!/usr/bin/env bash
# Changelog entries live one per file under changelog/unreleased/<kind>/, so
# parallel PRs never edit the same lines; CHANGELOG.md is written only when a
# release is cut. Format and rules: changelog/README.md.
#
#   changelog.sh new <kind> <slug>   start an entry (kind: added changed deprecated removed fixed security)
#   changelog.sh check               every entry is well-formed (the Policy check runs this)
#   changelog.sh preview             print the next release's notes as they stand
#   changelog.sh release [X.Y.Z]     cut a release: write the entries into CHANGELOG.md as
#                                    `## [next-vX.Y.Z] - <today>`, delete them, bump VERSION,
#                                    Cargo.toml and Cargo.lock. Without X.Y.Z: a minor bump when
#                                    anything was added, changed, deprecated or removed, else a
#                                    patch bump. Pass X.Y.Z for anything else (a breaking change).
#
# Plain bash 3.2 + POSIX tools, so it runs on a stock Mac and in CI without a build.
set -euo pipefail

root=$(git rev-parse --show-toplevel)
dir="$root/changelog/unreleased"
changelog="$root/CHANGELOG.md"
# Keep a Changelog order; each kind is a directory, titled in the release.
kinds="added changed deprecated removed fixed security"

die() { echo "changelog: $*" >&2; exit 1; }

title() {
  case "$1" in
    added) echo Added ;; changed) echo Changed ;; deprecated) echo Deprecated ;;
    removed) echo Removed ;; fixed) echo Fixed ;; security) echo Security ;;
    *) return 1 ;;
  esac
}

# Every entry file, oldest first by path.
entries() {
  [ -d "$dir" ] || return 0
  find "$dir" -type f ! -name .gitkeep | LC_ALL=C sort
}

# The entries of one kind, newest first (by the commit that added them; an
# uncommitted file counts as newest), so a release reads like the old file did.
entries_of() {
  local f t
  [ -d "$dir/$1" ] || return 0
  find "$dir/$1" -type f -name '*.md' | LC_ALL=C sort | while IFS= read -r f; do
    t=$(git -C "$root" log --diff-filter=A --format=%ct -1 -- "$f" 2>/dev/null || true)
    printf '%s\t%s\n' "${t:-9999999999}" "$f"
  done | LC_ALL=C sort -t "$(printf '\t')" -k1,1nr -k2,2 | cut -f2
}

check() {
  local f rel kind name problems=0 first
  while IFS= read -r f; do
    rel=${f#"$root"/}
    kind=$(basename "$(dirname "$f")")
    name=$(basename "$f")
    if [ "$(dirname "$(dirname "$f")")" != "$dir" ] || ! title "$kind" >/dev/null; then
      echo "::error file=$rel::$rel is not under changelog/unreleased/<kind>/ (kind: $kinds)"
      problems=$((problems + 1)); continue
    fi
    if [[ ! "$name" =~ ^[a-z0-9][a-z0-9.-]*\.md$ ]]; then
      echo "::error file=$rel::$rel: name it <slug>.md in lowercase, digits and dashes (e.g. 240-sent-view.md)"
      problems=$((problems + 1)); continue
    fi
    first=$(grep -m1 -v '^[[:space:]]*$' "$f" || true)
    if [ -z "$first" ]; then
      echo "::error file=$rel::$rel is empty"
      problems=$((problems + 1)); continue
    fi
    if [ "${first#- }" = "$first" ]; then
      echo "::error file=$rel::$rel must start with a list item ('- **What changed.** How it reads to a user.')"
      problems=$((problems + 1))
    fi
    if grep -q '^#' "$f"; then
      echo "::error file=$rel::$rel has a heading; the release adds them (the folder is the kind)"
      problems=$((problems + 1))
    fi
    if grep -q 'TODO(changelog)' "$f"; then
      echo "::error file=$rel::$rel still has the TODO(changelog) placeholder"
      problems=$((problems + 1))
    fi
  done < <(entries)
  [ "$problems" = 0 ] || die "$problems problem(s) in changelog/unreleased (changelog/README.md)"
  echo "changelog: $(entries | grep -c . || true) unreleased entr(y/ies) OK"
}

# The body of the next release: one `### Kind` per kind that has entries.
notes() {
  local kind f any=0 files
  for kind in $kinds; do
    files=$(entries_of "$kind")
    [ -n "$files" ] || continue
    [ "$any" = 0 ] || echo
    any=1
    printf '### %s\n\n' "$(title "$kind")"
    printf '%s\n' "$files" | while IFS= read -r f; do
      # Drop leading/trailing blank lines; keep the entry's own wrapping.
      sed -e '/./,$!d' "$f" | sed -e :a -e '/^\n*$/{$d;N;ba' -e '}'
    done
  done
}

bump() {
  local current=$1 major minor patch
  IFS=. read -r major minor patch <<EOF
$current
EOF
  if [ -n "$(entries_of added)$(entries_of changed)$(entries_of deprecated)$(entries_of removed)" ]; then
    echo "$major.$((minor + 1)).0"
  else
    echo "$major.$minor.$((patch + 1))"
  fi
}

release() {
  local current version tag body tmp
  current=$(tr -d '[:space:]' < "$root/VERSION")
  check >&2
  [ -n "$(entries)" ] || die "nothing to release: changelog/unreleased is empty"
  version=${1:-$(bump "$current")}
  [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "'$version' is not X.Y.Z"
  tag="next-v$version"
  if grep -qF "## [$tag]" "$changelog"; then
    die "CHANGELOG.md already has a '## [$tag]' section; pass a new version"
  fi

  body=$(notes)
  tmp=$(mktemp)
  # The new section goes above the newest release (or at the end of the intro).
  # ENVIRON, not -v: awk would read backslashes in the entries as escapes.
  HEAD="## [$tag] - $(date +%Y-%m-%d)" BODY="$body" awk '
    !done && /^## \[/ { print ENVIRON["HEAD"] "\n\n" ENVIRON["BODY"] "\n"; done = 1 }
    { print }
    END { if (!done) print "\n" ENVIRON["HEAD"] "\n\n" ENVIRON["BODY"] }
  ' "$changelog" > "$tmp"
  mv "$tmp" "$changelog"

  # Same version everywhere `cargo xtask arch` compares: VERSION, the
  # workspace package, and every workspace crate in the lockfile.
  printf '%s\n' "$version" > "$root/VERSION"
  tmp=$(mktemp)
  awk -v v="$version" '
    /^\[/ { section = $0 }
    section == "[workspace.package]" && /^version = / && !done { print "version = \"" v "\""; done = 1; next }
    { print }
  ' "$root/Cargo.toml" > "$tmp"
  mv "$tmp" "$root/Cargo.toml"
  tmp=$(mktemp)
  # A [[package]] without a `source` is a workspace member.
  awk -v old="$current" -v v="$version" '
    function flush() { for (i = 1; i <= n; i++) { if (!src && lines[i] == "version = \"" old "\"") lines[i] = "version = \"" v "\""; print lines[i] } n = 0; src = 0 }
    /^\[\[package\]\]/ { flush() }
    /^source = / { src = 1 }
    { lines[++n] = $0 }
    END { flush() }
  ' "$root/Cargo.lock" > "$tmp"
  mv "$tmp" "$root/Cargo.lock"

  entries | while IFS= read -r f; do git -C "$root" rm -q --cached --ignore-unmatch -- "$f" >/dev/null; rm -f "$f"; done
  find "$dir" -mindepth 1 -type d -empty -delete

  echo "changelog: released $tag ($current → $version). Review the diff, then open"
  echo "  'chore: release $tag' into next (CONTRIBUTING.md → Releases)."
}

new() {
  local kind=${1:-} slug=${2:-} f
  title "$kind" >/dev/null || die "usage: changelog.sh new <kind> <slug> (kind: $kinds)"
  [[ "$slug" =~ ^[a-z0-9][a-z0-9.-]*$ ]] || die "slug: lowercase, digits and dashes, e.g. 240-sent-view"
  f="$dir/$kind/$slug.md"
  [ ! -e "$f" ] || die "${f#"$root"/} already exists"
  mkdir -p "$dir/$kind"
  cat > "$f" <<'EOF'
- **TODO(changelog) What changed, in a few words.** What a user or self-hoster
  notices, in plain language. Wrap at about 90 columns.
EOF
  echo "${f#"$root"/}"
}

case "${1:-}" in
  new) shift; new "$@" ;;
  check) check ;;
  preview) notes ;;
  release) shift; release "$@" ;;
  *) sed -n '2,15p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 64 ;;
esac

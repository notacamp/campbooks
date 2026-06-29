# Campbooks CLI

`campbooks` is the developer CLI for Campbooks — drive your inbox, documents,
contacts, tags, and Scout from the terminal. It's a single static Go binary that
wraps the [public REST API](api.md).

## Install

```sh
brew install notacamp/tap/campbooks
```

Or build from source (Go 1.20+):

```sh
go install github.com/notacamp/campbooks/cli@latest
```

## Sign in

The default is **browser SSO** — no API keys to copy:

```sh
campbooks login
```

This opens your browser, you sign in with your normal Campbooks session and
approve the "Campbooks CLI" client, and the CLI stores a token (it refreshes
automatically). Point it at a self-hosted instance with `--host`:

```sh
campbooks login --host campbooks.example.com
```

Other modes:

| Command | Use |
|---------|-----|
| `campbooks login` | Browser SSO (authorization_code + PKCE) |
| `campbooks login --no-browser` | Print a URL, paste the code back (no browser available) |
| `campbooks login --client-id … --client-secret …` | Headless / CI (a client-credentials client from **Settings → API access**) |

`campbooks whoami` shows who you're signed in as. Credentials live in
`~/.config/campbooks/config.yml` (mode `0600`); override the directory with
`CAMPBOOKS_CONFIG_DIR`.

### Multiple hosts

The CLI talks to several deployments side by side (e.g. cloud + self-hosted):

```sh
campbooks config list                 # all hosts, signed-in status, default
campbooks config use campbooks.example.com
campbooks logout                      # sign out of the current host
```

Pass `--host <host>` to any command to target a specific one.

## Output

Every command prints a human-friendly table by default. Add `--json` for raw,
script-friendly JSON; `NO_COLOR` disables color.

```sh
campbooks emails list --unread --json | jq '.[].subject'
```

## Commands

Run `campbooks <group> --help` for full flags. Highlights:

### Email

```sh
campbooks emails list --unread --category finance -q invoice
campbooks emails get 42
campbooks emails send --account 3 --to a@b.com --subject "Hi" --body "..."
campbooks emails send --account 3 --to a@b.com --body-file ./note.html
campbooks emails reply 42 --body "Thanks!"
campbooks emails read 42        # · unread 42
campbooks emails tag 42 receipts   # · untag 42 <tag-id>
```

### Documents

```sh
campbooks documents list --review-status pending --ai-status completed
campbooks documents get 88
campbooks documents download 88 -o invoice.pdf
campbooks documents upload invoice.pdf receipt.png   # AI processing is async
campbooks documents update 88 --vendor-name ACME --amount-cents 12900
campbooks documents update 88 --set tax_rate=23 --set period_start=2026-06-01
campbooks documents approve 88   # · reject 88
campbooks documents reclassify 88 --type 3
```

### Contacts

```sh
campbooks contacts list --list-status blocked --starred
campbooks contacts get 5
campbooks contacts update 5 --relationship-type client
campbooks contacts star 5   # · unstar · allow · block · unblock
```

### Tags & document types

```sh
campbooks tags list
campbooks doctypes list
```

### Scout (AI assistant)

Scout is asynchronous: a message returns immediately and the reply is generated
in the background. `--wait` polls for it.

```sh
campbooks scout ask "What needs my attention today?" --wait
campbooks scout ask "Summarize thread 7" --thread 7
campbooks scout threads list
campbooks scout threads new --title "Tax prep"
campbooks scout messages 7
```

## Scopes & permissions

`campbooks login` requests the full scope set. Scopes are only a ceiling — every
request is still gated by your own permissions (you can only send from accounts
you may send from, see emails you may see, etc.). For finer-grained tokens, use a
client-credentials client with a narrower scope selection.

## Develop

The CLI lives in [`cli/`](../cli) (a self-contained Go module).

```sh
cd cli
go build ./... && go vet ./... && go test ./...
```

Releases are cut from `cli/vX.Y.Z` tags (GoReleaser → GitHub Release + Homebrew
tap), separate from the app's `vX.Y.Z` image releases.

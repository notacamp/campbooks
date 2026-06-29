# campbooks CLI

The developer CLI for [Campbooks](https://github.com/notacamp/campbooks) — drive
your inbox, documents, contacts, tags, and Scout from the terminal over the
[public REST API](../docs/api.md). A single static Go binary.

## Install

```sh
brew install notacamp/campbooks/campbooks   # (released binaries)
# or build from source:
go install github.com/notacamp/campbooks/cli@latest
```

## Sign in

```sh
campbooks login                          # browser SSO (authorization_code + PKCE)
campbooks login --host campbooks.example.com
campbooks login --no-browser             # print a URL, paste the code (no browser)
campbooks login --client-id … --client-secret …   # headless / CI (client_credentials)
campbooks whoami
```

`login` opens your browser, you sign in with your normal Campbooks session and
approve the "Campbooks CLI" client, and the CLI stores a token in
`~/.config/campbooks/config.yml` (0600). The token refreshes automatically.

The CLI is multi-host (cloud + self-hosted side by side):

```sh
campbooks config list
campbooks config use campbooks.example.com
campbooks logout
```

## Output

Commands print human-friendly tables by default; add `--json` for raw,
script-friendly JSON. `NO_COLOR` is respected.

```sh
campbooks --help
```

## Develop

```sh
cd cli
go build ./... && go vet ./... && go test ./...
```

Layout: `internal/config` (on-disk hosts + tokens), `internal/auth` (OAuth flows
+ PKCE loopback login), `internal/client` (typed `/api/v1` client with
auto-refresh), `internal/output` (table vs JSON), `internal/cmd` (cobra tree).

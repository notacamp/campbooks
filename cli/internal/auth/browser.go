package auth

import (
	"bufio"
	"context"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/url"
	"strings"

	"github.com/notacamp/campbooks/cli/internal/config"
	"github.com/pkg/browser"
)

func authorizeURL(endpoint, redirectURI, scope, state, verifier string) string {
	return endpoint + authorizePath + "?" + url.Values{
		"response_type":         {"code"},
		"client_id":             {CLIClientID},
		"redirect_uri":          {redirectURI},
		"scope":                 {scope},
		"state":                 {state},
		"code_challenge":        {challengeS256(verifier)},
		"code_challenge_method": {"S256"},
	}.Encode()
}

// BrowserLogin runs the authorization_code + PKCE flow: it starts a loopback
// server on an ephemeral port, opens the browser to the consent screen, captures
// the redirected code, and exchanges it (with the PKCE verifier) for tokens.
// openFn defaults to opening the system browser; it is injectable for tests.
func BrowserLogin(ctx context.Context, hc *http.Client, endpoint string, scopes []string, out io.Writer, openFn func(string) error) (*config.Auth, error) {
	if openFn == nil {
		openFn = browser.OpenURL
	}
	verifier, err := newVerifier()
	if err != nil {
		return nil, err
	}
	state, err := randomState()
	if err != nil {
		return nil, err
	}

	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		return nil, fmt.Errorf("starting loopback listener: %w", err)
	}
	defer ln.Close()
	redirectURI := fmt.Sprintf("http://127.0.0.1:%d/callback", ln.Addr().(*net.TCPAddr).Port)

	type result struct {
		code string
		err  error
	}
	resCh := make(chan result, 1)
	srv := &http.Server{Handler: http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/callback" {
			http.NotFound(w, r)
			return
		}
		q := r.URL.Query()
		switch {
		case q.Get("error") != "":
			writeBrowserPage(w, false, q.Get("error_description"))
			resCh <- result{err: fmt.Errorf("authorization denied: %s", q.Get("error"))}
		case q.Get("state") != state:
			writeBrowserPage(w, false, "state mismatch")
			resCh <- result{err: fmt.Errorf("state mismatch — possible CSRF, aborting")}
		default:
			writeBrowserPage(w, true, "")
			resCh <- result{code: q.Get("code")}
		}
	})}
	go srv.Serve(ln)
	defer srv.Close()

	authURL := authorizeURL(endpoint, redirectURI, strings.Join(scopes, " "), state, verifier)
	fmt.Fprintln(out, "Opening your browser to sign in…")
	fmt.Fprintln(out, "If it doesn't open automatically, visit:\n  "+authURL)
	_ = openFn(authURL)

	select {
	case <-ctx.Done():
		return nil, ctx.Err()
	case res := <-resCh:
		if res.err != nil {
			return nil, res.err
		}
		return exchangeCode(ctx, hc, endpoint, res.code, redirectURI, verifier)
	}
}

// OOBLogin is the no-browser fallback: print the authorize URL (with the OOB
// redirect) and read the code the user pastes back.
func OOBLogin(ctx context.Context, hc *http.Client, endpoint string, scopes []string, in io.Reader, out io.Writer) (*config.Auth, error) {
	verifier, err := newVerifier()
	if err != nil {
		return nil, err
	}
	state, err := randomState()
	if err != nil {
		return nil, err
	}
	authURL := authorizeURL(endpoint, oobRedirect, strings.Join(scopes, " "), state, verifier)
	fmt.Fprintln(out, "Open this URL in a browser, approve access, then paste the code below:")
	fmt.Fprintln(out, "  "+authURL)
	fmt.Fprint(out, "\nCode: ")

	sc := bufio.NewScanner(in)
	if !sc.Scan() {
		return nil, fmt.Errorf("no code entered")
	}
	code := strings.TrimSpace(sc.Text())
	if code == "" {
		return nil, fmt.Errorf("no code entered")
	}
	return exchangeCode(ctx, hc, endpoint, code, oobRedirect, verifier)
}

func exchangeCode(ctx context.Context, hc *http.Client, endpoint, code, redirectURI, verifier string) (*config.Auth, error) {
	tr, err := postForm(ctx, hc, endpoint, tokenPath, url.Values{
		"grant_type":    {"authorization_code"},
		"code":          {code},
		"client_id":     {CLIClientID},
		"redirect_uri":  {redirectURI},
		"code_verifier": {verifier},
	})
	if err != nil {
		return nil, err
	}
	return toAuth(MethodSSO, tr), nil
}

func writeBrowserPage(w http.ResponseWriter, ok bool, detail string) {
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	title, msg := "You're signed in", "You can close this tab and return to your terminal."
	if !ok {
		title, msg = "Sign-in failed", "Return to your terminal and try again."
		if detail != "" {
			msg = detail
		}
	}
	fmt.Fprintf(w, `<!doctype html><html lang="en"><head><meta charset="utf-8">
<title>%s · Campbooks CLI</title><style>
body{font-family:-apple-system,system-ui,sans-serif;background:#fafafa;color:#111;display:flex;min-height:100vh;align-items:center;justify-content:center;margin:0}
.card{max-width:24rem;padding:2rem 2.5rem;border:1px solid #e5e5e5;border-radius:1rem;text-align:center;background:#fff;box-shadow:0 1px 3px rgba(0,0,0,.06)}
h1{font-size:1.25rem;margin:0 0 .5rem}p{color:#555;margin:0;line-height:1.5}
</style></head><body><div class="card"><h1>%s</h1><p>%s</p></div></body></html>`, title, title, msg)
}

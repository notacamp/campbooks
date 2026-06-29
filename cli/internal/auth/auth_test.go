package auth

import (
	"context"
	"io"
	"net/http"
	"net/http/httptest"
	"net/url"
	"testing"

	"github.com/notacamp/campbooks/cli/internal/config"
)

func TestChallengeS256_RFC7636Vector(t *testing.T) {
	// RFC 7636 Appendix B worked example.
	verifier := "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"
	want := "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
	if got := challengeS256(verifier); got != want {
		t.Errorf("challengeS256 = %q, want %q", got, want)
	}
}

func TestNewVerifierLength(t *testing.T) {
	v, err := newVerifier()
	if err != nil {
		t.Fatal(err)
	}
	if len(v) < 43 || len(v) > 128 {
		t.Errorf("verifier length %d outside PKCE 43..128", len(v))
	}
}

func fakeTokenServer(handle func(grant string, form url.Values) (int, string)) *httptest.Server {
	return httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != tokenPath {
			http.NotFound(w, r)
			return
		}
		body, _ := io.ReadAll(r.Body)
		form, _ := url.ParseQuery(string(body))
		code, resp := handle(form.Get("grant_type"), form)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(code)
		_, _ = io.WriteString(w, resp)
	}))
}

func TestClientCredentialsLogin(t *testing.T) {
	srv := fakeTokenServer(func(grant string, form url.Values) (int, string) {
		if grant != "client_credentials" || form.Get("client_id") != "cid" || form.Get("client_secret") != "sec" {
			return 401, `{"error":{"code":"invalid_client","message":"bad"}}`
		}
		return 200, `{"access_token":"AT","token_type":"Bearer","expires_in":7200,"scope":"emails:read"}`
	})
	defer srv.Close()

	a, err := ClientCredentialsLogin(context.Background(), srv.Client(), srv.URL, "cid", "sec", []string{"emails:read"})
	if err != nil {
		t.Fatal(err)
	}
	if a.AccessToken != "AT" || a.Method != MethodClientCredentials || a.ClientID != "cid" || a.ClientSecret != "sec" {
		t.Errorf("auth = %+v", a)
	}
	if a.ExpiresAt == 0 {
		t.Error("expected ExpiresAt to be set")
	}
}

func TestClientCredentialsLogin_BadCreds(t *testing.T) {
	srv := fakeTokenServer(func(string, url.Values) (int, string) {
		return 401, `{"error":{"code":"invalid_client","message":"bad creds"}}`
	})
	defer srv.Close()
	_, err := ClientCredentialsLogin(context.Background(), srv.Client(), srv.URL, "x", "y", nil)
	if err == nil {
		t.Fatal("expected error")
	}
}

func TestRefreshSSO(t *testing.T) {
	srv := fakeTokenServer(func(grant string, form url.Values) (int, string) {
		if grant != "refresh_token" || form.Get("refresh_token") != "RT" || form.Get("client_id") != CLIClientID {
			return 400, `{"error":{"code":"invalid_grant","message":"no"}}`
		}
		return 200, `{"access_token":"AT2","refresh_token":"RT2","token_type":"Bearer","expires_in":7200}`
	})
	defer srv.Close()

	na, err := Refresh(context.Background(), srv.Client(), srv.URL, &config.Auth{Method: MethodSSO, AccessToken: "AT", RefreshToken: "RT"})
	if err != nil {
		t.Fatal(err)
	}
	if na.AccessToken != "AT2" || na.RefreshToken != "RT2" {
		t.Errorf("refreshed = %+v", na)
	}
}

func TestBrowserLoginHappyPath(t *testing.T) {
	srv := fakeTokenServer(func(grant string, form url.Values) (int, string) {
		if grant != "authorization_code" || form.Get("code") != "the_code" || form.Get("code_verifier") == "" || form.Get("client_id") != CLIClientID {
			return 400, `{"error":{"code":"invalid_grant","message":"bad code/verifier"}}`
		}
		return 200, `{"access_token":"AT","refresh_token":"RT","token_type":"Bearer","expires_in":7200,"scope":"emails:read"}`
	})
	defer srv.Close()

	// Stand in for the browser: given the authorize URL, hit the loopback
	// callback with a code and the matching state.
	open := func(u string) error {
		parsed, err := url.Parse(u)
		if err != nil {
			return err
		}
		q := parsed.Query()
		go func() {
			resp, err := http.Get(q.Get("redirect_uri") + "?code=the_code&state=" + q.Get("state"))
			if err == nil {
				resp.Body.Close()
			}
		}()
		return nil
	}

	a, err := BrowserLogin(context.Background(), srv.Client(), srv.URL, []string{"emails:read"}, io.Discard, open)
	if err != nil {
		t.Fatal(err)
	}
	if a.AccessToken != "AT" || a.RefreshToken != "RT" || a.Method != MethodSSO {
		t.Errorf("auth = %+v", a)
	}
}

func TestBrowserLoginStateMismatch(t *testing.T) {
	srv := fakeTokenServer(func(string, url.Values) (int, string) { return 200, `{"access_token":"AT"}` })
	defer srv.Close()
	open := func(u string) error {
		parsed, _ := url.Parse(u)
		go func() {
			resp, err := http.Get(parsed.Query().Get("redirect_uri") + "?code=x&state=WRONG")
			if err == nil {
				resp.Body.Close()
			}
		}()
		return nil
	}
	if _, err := BrowserLogin(context.Background(), srv.Client(), srv.URL, nil, io.Discard, open); err == nil {
		t.Fatal("expected state-mismatch error")
	}
}

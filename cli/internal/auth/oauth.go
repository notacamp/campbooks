// Package auth implements the CLI's OAuth flows against a Campbooks deployment:
// the browser authorization_code + PKCE login (the default), the headless
// client_credentials login, and token refresh/revocation. It talks to the same
// /api/oauth endpoints the web app exposes.
package auth

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
	"time"

	"github.com/notacamp/campbooks/cli/internal/config"
)

const (
	// CLIClientID is the well-known, first-party public OAuth client present on
	// every Campbooks deployment (server side: Api::CliApplication). It's public —
	// safe to embed — because the authorization_code flow is protected by PKCE,
	// not a client secret.
	CLIClientID = "campbooks-cli"

	authorizePath = "/api/oauth/authorize"
	tokenPath     = "/api/oauth/token"
	revokePath    = "/api/oauth/revoke"

	oobRedirect = "urn:ietf:wg:oauth:2.0:oob"

	// MethodSSO and MethodClientCredentials tag how an Auth was obtained, so the
	// CLI knows how to refresh it.
	MethodSSO               = "sso"
	MethodClientCredentials = "client_credentials"
)

// DefaultScopes are requested on `campbooks login` — everything the signed-in
// user may do. Scope is only a ceiling; the user's own permissions still apply.
var DefaultScopes = []string{
	"emails:read", "emails:write", "emails:send",
	"documents:read", "documents:write",
	"contacts:read", "contacts:write",
	"tags:read", "tags:write",
	"document_types:read",
	"scout:read", "scout:write",
}

// tokenResponse is Doorkeeper's POST /api/oauth/token body.
type tokenResponse struct {
	AccessToken  string `json:"access_token"`
	TokenType    string `json:"token_type"`
	ExpiresIn    int64  `json:"expires_in"`
	RefreshToken string `json:"refresh_token"`
	Scope        string `json:"scope"`
}

type oauthError struct {
	Err struct {
		Code    string `json:"code"`
		Message string `json:"message"`
	} `json:"error"`
}

func postForm(ctx context.Context, hc *http.Client, endpoint, path string, form url.Values) (*tokenResponse, error) {
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, endpoint+path, strings.NewReader(form.Encode()))
	if err != nil {
		return nil, err
	}
	req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	req.Header.Set("Accept", "application/json")

	resp, err := hc.Do(req)
	if err != nil {
		return nil, fmt.Errorf("contacting %s: %w", endpoint, err)
	}
	defer resp.Body.Close()

	body, _ := io.ReadAll(io.LimitReader(resp.Body, 1<<20))
	if resp.StatusCode != http.StatusOK {
		var oe oauthError
		if json.Unmarshal(body, &oe) == nil && oe.Err.Message != "" {
			return nil, fmt.Errorf("%s: %s", oe.Err.Code, oe.Err.Message)
		}
		return nil, fmt.Errorf("token endpoint returned %s", resp.Status)
	}
	var tr tokenResponse
	if err := json.Unmarshal(body, &tr); err != nil {
		return nil, fmt.Errorf("decoding token response: %w", err)
	}
	return &tr, nil
}

func toAuth(method string, tr *tokenResponse) *config.Auth {
	a := &config.Auth{
		Method:       method,
		AccessToken:  tr.AccessToken,
		RefreshToken: tr.RefreshToken,
		TokenType:    tr.TokenType,
		Scope:        tr.Scope,
	}
	if tr.ExpiresIn > 0 {
		a.ExpiresAt = time.Now().Add(time.Duration(tr.ExpiresIn) * time.Second).Unix()
	}
	return a
}

// ClientCredentialsLogin exchanges a client_id/secret for a token (headless/CI).
func ClientCredentialsLogin(ctx context.Context, hc *http.Client, endpoint, clientID, clientSecret string, scopes []string) (*config.Auth, error) {
	tr, err := postForm(ctx, hc, endpoint, tokenPath, url.Values{
		"grant_type":    {"client_credentials"},
		"client_id":     {clientID},
		"client_secret": {clientSecret},
		"scope":         {strings.Join(scopes, " ")},
	})
	if err != nil {
		return nil, err
	}
	a := toAuth(MethodClientCredentials, tr)
	a.ClientID, a.ClientSecret = clientID, clientSecret
	return a, nil
}

// Refresh returns a fresh Auth for an expiring one. SSO logins use the refresh
// token; client_credentials logins re-mint from the stored client_id/secret.
func Refresh(ctx context.Context, hc *http.Client, endpoint string, a *config.Auth) (*config.Auth, error) {
	switch a.Method {
	case MethodSSO:
		if a.RefreshToken == "" {
			return nil, fmt.Errorf("session expired — run `campbooks login` again")
		}
		tr, err := postForm(ctx, hc, endpoint, tokenPath, url.Values{
			"grant_type":    {"refresh_token"},
			"refresh_token": {a.RefreshToken},
			"client_id":     {CLIClientID},
		})
		if err != nil {
			return nil, err
		}
		na := toAuth(MethodSSO, tr)
		if na.RefreshToken == "" { // server didn't rotate it
			na.RefreshToken = a.RefreshToken
		}
		return na, nil
	case MethodClientCredentials:
		return ClientCredentialsLogin(ctx, hc, endpoint, a.ClientID, a.ClientSecret, strings.Fields(a.Scope))
	default:
		return nil, fmt.Errorf("unknown auth method %q", a.Method)
	}
}

// Revoke best-effort revokes the access token (used by `campbooks logout`).
func Revoke(ctx context.Context, hc *http.Client, endpoint string, a *config.Auth) error {
	if a == nil || a.AccessToken == "" {
		return nil
	}
	form := url.Values{"token": {a.AccessToken}, "client_id": {CLIClientID}}
	if a.Method == MethodClientCredentials {
		form.Set("client_id", a.ClientID)
		form.Set("client_secret", a.ClientSecret)
	}
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, endpoint+revokePath, strings.NewReader(form.Encode()))
	if err != nil {
		return err
	}
	req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	resp, err := hc.Do(req)
	if err != nil {
		return err
	}
	return resp.Body.Close()
}

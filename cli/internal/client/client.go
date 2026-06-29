// Package client is an authenticated HTTP client for the Campbooks public REST
// API (/api/v1) on one host. It transparently refreshes the access token when it
// is expiring or a request returns 401, persisting the new token via onRefresh,
// and decodes the { data, meta } / { error } envelopes.
package client

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"mime/multipart"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/notacamp/campbooks/cli/internal/auth"
	"github.com/notacamp/campbooks/cli/internal/config"
)

// Client talks to /api/v1 on one Campbooks host.
type Client struct {
	Endpoint  string
	HTTP      *http.Client
	auth      *config.Auth
	onRefresh func(*config.Auth) error
}

// New builds a client. onRefresh (may be nil) is called whenever the token is
// refreshed, so the caller can persist it.
func New(endpoint string, a *config.Auth, onRefresh func(*config.Auth) error) *Client {
	return &Client{
		Endpoint:  strings.TrimRight(endpoint, "/"),
		HTTP:      &http.Client{Timeout: 90 * time.Second},
		auth:      a,
		onRefresh: onRefresh,
	}
}

// Auth returns the current credentials (for `whoami`).
func (c *Client) Auth() *config.Auth { return c.auth }

// Meta is the pagination block on list responses.
type Meta struct {
	Page       int `json:"page"`
	PerPage    int `json:"per_page"`
	Total      int `json:"total"`
	TotalPages int `json:"total_pages"`
}

// APIError is a non-2xx { error: { code, message, details } } response.
type APIError struct {
	StatusCode int
	Code       string          `json:"code"`
	Message    string          `json:"message"`
	Details    json.RawMessage `json:"details,omitempty"`
}

func (e *APIError) Error() string {
	if e.Message != "" {
		if e.Code != "" {
			return fmt.Sprintf("%s (%s)", e.Message, e.Code)
		}
		return e.Message
	}
	return fmt.Sprintf("request failed with status %d", e.StatusCode)
}

type request struct {
	method      string
	path        string
	query       url.Values
	body        []byte
	contentType string
	accept      string
}

func (c *Client) send(ctx context.Context, r request) (*http.Response, error) {
	u := c.Endpoint + r.path
	if len(r.query) > 0 {
		u += "?" + r.query.Encode()
	}
	var body io.Reader
	if r.body != nil {
		body = bytes.NewReader(r.body)
	}
	req, err := http.NewRequestWithContext(ctx, r.method, u, body)
	if err != nil {
		return nil, err
	}
	if c.auth != nil && c.auth.AccessToken != "" {
		req.Header.Set("Authorization", "Bearer "+c.auth.AccessToken)
	}
	if r.contentType != "" {
		req.Header.Set("Content-Type", r.contentType)
	}
	accept := r.accept
	if accept == "" {
		accept = "application/json"
	}
	req.Header.Set("Accept", accept)
	return c.HTTP.Do(req)
}

// doRaw sends the request, refreshing the token first if it's expiring and once
// more if the server still answers 401.
func (c *Client) doRaw(ctx context.Context, r request) (*http.Response, error) {
	if err := c.ensureFresh(ctx); err != nil {
		return nil, err
	}
	resp, err := c.send(ctx, r)
	if err != nil {
		return nil, err
	}
	if resp.StatusCode == http.StatusUnauthorized {
		resp.Body.Close()
		if rerr := c.refresh(ctx); rerr != nil {
			return nil, rerr
		}
		return c.send(ctx, r)
	}
	return resp, nil
}

func (c *Client) do(ctx context.Context, r request) ([]byte, *http.Response, error) {
	resp, err := c.doRaw(ctx, r)
	if err != nil {
		return nil, nil, err
	}
	defer resp.Body.Close()
	body, _ := io.ReadAll(resp.Body)
	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		return nil, resp, parseAPIError(resp.StatusCode, body)
	}
	return body, resp, nil
}

func (c *Client) ensureFresh(ctx context.Context) error {
	if c.auth == nil || c.auth.ExpiresAt == 0 {
		return nil
	}
	if time.Now().Unix() < c.auth.ExpiresAt-60 {
		return nil
	}
	return c.refresh(ctx)
}

func (c *Client) refresh(ctx context.Context) error {
	if c.auth == nil {
		return fmt.Errorf("not logged in — run `campbooks login`")
	}
	na, err := auth.Refresh(ctx, c.HTTP, c.Endpoint, c.auth)
	if err != nil {
		return err
	}
	c.auth = na
	if c.onRefresh != nil {
		return c.onRefresh(na)
	}
	return nil
}

func parseAPIError(status int, body []byte) error {
	var env struct {
		Error APIError `json:"error"`
	}
	if json.Unmarshal(body, &env) == nil && (env.Error.Code != "" || env.Error.Message != "") {
		env.Error.StatusCode = status
		return &env.Error
	}
	msg := strings.TrimSpace(string(body))
	if len(msg) > 200 {
		msg = msg[:200]
	}
	return &APIError{StatusCode: status, Message: msg}
}

func decodeEnvelope(body []byte) (json.RawMessage, *Meta, error) {
	var env struct {
		Data json.RawMessage `json:"data"`
		Meta *Meta           `json:"meta"`
	}
	if err := json.Unmarshal(body, &env); err != nil {
		return nil, nil, fmt.Errorf("decoding response: %w", err)
	}
	return env.Data, env.Meta, nil
}

// Get fetches a JSON endpoint and returns the raw `data` plus `meta` (nil for
// unpaginated endpoints).
func (c *Client) Get(ctx context.Context, path string, query url.Values) (json.RawMessage, *Meta, error) {
	body, _, err := c.do(ctx, request{method: http.MethodGet, path: path, query: query})
	if err != nil {
		return nil, nil, err
	}
	return decodeEnvelope(body)
}

// Send performs a write (POST/PATCH/DELETE) with an optional form body and
// returns the `data` of the response (nil for 204/empty bodies).
func (c *Client) Send(ctx context.Context, method, path string, query, form url.Values) (json.RawMessage, error) {
	r := request{method: method, path: path, query: query}
	if form != nil {
		r.body = []byte(form.Encode())
		r.contentType = "application/x-www-form-urlencoded"
	}
	body, resp, err := c.do(ctx, r)
	if err != nil {
		return nil, err
	}
	if resp.StatusCode == http.StatusNoContent || len(bytes.TrimSpace(body)) == 0 {
		return nil, nil
	}
	data, _, err := decodeEnvelope(body)
	return data, err
}

// Upload multipart-POSTs one or more files under fieldName, with optional extra
// form fields, and returns the response `data`.
func (c *Client) Upload(ctx context.Context, path, fieldName string, files []string, extra url.Values) (json.RawMessage, error) {
	var buf bytes.Buffer
	mw := multipart.NewWriter(&buf)
	for _, f := range files {
		fh, err := os.Open(f)
		if err != nil {
			return nil, err
		}
		part, err := mw.CreateFormFile(fieldName, filepath.Base(f))
		if err != nil {
			fh.Close()
			return nil, err
		}
		if _, err := io.Copy(part, fh); err != nil {
			fh.Close()
			return nil, err
		}
		fh.Close()
	}
	for k, vs := range extra {
		for _, v := range vs {
			_ = mw.WriteField(k, v)
		}
	}
	if err := mw.Close(); err != nil {
		return nil, err
	}
	body, _, err := c.do(ctx, request{method: http.MethodPost, path: path, body: buf.Bytes(), contentType: mw.FormDataContentType()})
	if err != nil {
		return nil, err
	}
	data, _, err := decodeEnvelope(body)
	return data, err
}

// Download streams a binary endpoint (e.g. a document file). The caller must
// close the returned body.
func (c *Client) Download(ctx context.Context, path string) (*http.Response, error) {
	resp, err := c.doRaw(ctx, request{method: http.MethodGet, path: path, accept: "*/*"})
	if err != nil {
		return nil, err
	}
	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		b, _ := io.ReadAll(io.LimitReader(resp.Body, 1<<20))
		resp.Body.Close()
		return nil, parseAPIError(resp.StatusCode, b)
	}
	return resp, nil
}

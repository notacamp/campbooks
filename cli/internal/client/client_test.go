package client

import (
	"context"
	"io"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/notacamp/campbooks/cli/internal/config"
)

func TestGetEnvelope(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Header.Get("Authorization") != "Bearer tok" {
			w.WriteHeader(http.StatusUnauthorized)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		_, _ = io.WriteString(w, `{"data":[{"id":1}],"meta":{"page":1,"per_page":25,"total":1,"total_pages":1}}`)
	}))
	defer srv.Close()

	c := New(srv.URL, &config.Auth{AccessToken: "tok"}, nil)
	data, meta, err := c.Get(context.Background(), "/api/v1/emails", nil)
	if err != nil {
		t.Fatal(err)
	}
	if meta == nil || meta.Total != 1 || meta.Page != 1 {
		t.Errorf("meta = %+v", meta)
	}
	if string(data) != `[{"id":1}]` {
		t.Errorf("data = %s", data)
	}
}

func TestAPIError(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusNotFound)
		_, _ = io.WriteString(w, `{"error":{"code":"not_found","message":"Resource not found."}}`)
	}))
	defer srv.Close()

	c := New(srv.URL, &config.Auth{AccessToken: "tok"}, nil)
	_, _, err := c.Get(context.Background(), "/api/v1/emails/999", nil)
	ae, ok := err.(*APIError)
	if !ok {
		t.Fatalf("expected *APIError, got %T: %v", err, err)
	}
	if ae.Code != "not_found" || ae.StatusCode != 404 {
		t.Errorf("APIError = %+v", ae)
	}
}

func TestRefreshOn401(t *testing.T) {
	mux := http.NewServeMux()
	mux.HandleFunc("/api/oauth/token", func(w http.ResponseWriter, _ *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		_, _ = io.WriteString(w, `{"access_token":"NEW","refresh_token":"RT2","token_type":"Bearer","expires_in":7200}`)
	})
	mux.HandleFunc("/api/v1/emails", func(w http.ResponseWriter, r *http.Request) {
		if r.Header.Get("Authorization") != "Bearer NEW" {
			w.WriteHeader(http.StatusUnauthorized)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		_, _ = io.WriteString(w, `{"data":[],"meta":{"page":1,"per_page":25,"total":0,"total_pages":0}}`)
	})
	srv := httptest.NewServer(mux)
	defer srv.Close()

	var persisted *config.Auth
	c := New(srv.URL, &config.Auth{
		Method:       "sso",
		AccessToken:  "OLD",
		RefreshToken: "RT",
		ExpiresAt:    time.Now().Add(time.Hour).Unix(), // not yet expiring; only the 401 triggers refresh
	}, func(a *config.Auth) error {
		persisted = a
		return nil
	})

	_, meta, err := c.Get(context.Background(), "/api/v1/emails", nil)
	if err != nil {
		t.Fatal(err)
	}
	if meta == nil {
		t.Fatal("expected meta after refresh+retry")
	}
	if persisted == nil || persisted.AccessToken != "NEW" {
		t.Errorf("expected refreshed token persisted, got %+v", persisted)
	}
}

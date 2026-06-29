package config

import (
	"os"
	"testing"
)

func TestNormalizeEndpoint(t *testing.T) {
	cases := []struct{ in, want string }{
		{"app.example.com", "https://app.example.com"},
		{"https://app.example.com/", "https://app.example.com"},
		{"localhost:3000", "http://localhost:3000"},
		{"127.0.0.1:3000", "http://127.0.0.1:3000"},
		{"http://localhost:3000/api/", "http://localhost:3000/api"},
		{"https://x.example.com?a=1#f", "https://x.example.com"},
	}
	for _, c := range cases {
		got, err := NormalizeEndpoint(c.in)
		if err != nil {
			t.Fatalf("NormalizeEndpoint(%q): %v", c.in, err)
		}
		if got != c.want {
			t.Errorf("NormalizeEndpoint(%q) = %q, want %q", c.in, got, c.want)
		}
	}
	if _, err := NormalizeEndpoint(""); err == nil {
		t.Error("expected error for empty endpoint")
	}
}

func TestHostKey(t *testing.T) {
	k, err := HostKey("https://app.example.com")
	if err != nil || k != "app.example.com" {
		t.Fatalf("HostKey = %q, %v", k, err)
	}
	k, _ = HostKey("http://localhost:3000")
	if k != "localhost:3000" {
		t.Errorf("HostKey loopback = %q", k)
	}
}

func TestSaveLoadRoundTrip(t *testing.T) {
	dir := t.TempDir()
	t.Setenv("CAMPBOOKS_CONFIG_DIR", dir)

	cfg, err := Load()
	if err != nil {
		t.Fatal(err)
	}
	key, host, err := cfg.UpsertHost("https://app.example.com/")
	if err != nil {
		t.Fatal(err)
	}
	if key != "app.example.com" {
		t.Fatalf("key = %q", key)
	}
	host.Auth = &Auth{Method: "sso", AccessToken: "tok", RefreshToken: "ref"}
	if err := cfg.Save(); err != nil {
		t.Fatal(err)
	}

	// File must be 0600 (it holds tokens).
	info, err := os.Stat(dir + "/config.yml")
	if err != nil {
		t.Fatal(err)
	}
	if perm := info.Mode().Perm(); perm != 0o600 {
		t.Errorf("config perms = %o, want 600", perm)
	}

	reloaded, err := Load()
	if err != nil {
		t.Fatal(err)
	}
	if reloaded.DefaultHost != "app.example.com" {
		t.Errorf("DefaultHost = %q", reloaded.DefaultHost)
	}
	h := reloaded.Hosts["app.example.com"]
	if h == nil || h.Auth == nil || h.Auth.AccessToken != "tok" || h.Auth.RefreshToken != "ref" {
		t.Errorf("auth not round-tripped: %+v", h)
	}
}

func TestResolve(t *testing.T) {
	cfg := &Config{Hosts: map[string]*Host{}}
	if _, _, err := cfg.Resolve(""); err == nil {
		t.Error("expected error with no hosts")
	}
	cfg.UpsertHost("https://a.example.com")
	cfg.UpsertHost("https://b.example.com")
	// explicit by host key
	if k, _, err := cfg.Resolve("b.example.com"); err != nil || k != "b.example.com" {
		t.Errorf("Resolve explicit = %q, %v", k, err)
	}
	// explicit by URL
	if k, _, err := cfg.Resolve("https://b.example.com"); err != nil || k != "b.example.com" {
		t.Errorf("Resolve url = %q, %v", k, err)
	}
	// default
	if k, _, err := cfg.Resolve(""); err != nil || k != "a.example.com" {
		t.Errorf("Resolve default = %q, %v", k, err)
	}
	if _, _, err := cfg.Resolve("nope.example.com"); err == nil {
		t.Error("expected error for unknown host")
	}
}

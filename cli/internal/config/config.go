// Package config persists the CLI's per-host settings and credentials in a
// single 0600 file (gh-style), default ~/.config/campbooks/config.yml. A "host"
// is the host[:port] of a Campbooks deployment, so one install can talk to the
// cloud and a self-hosted instance side by side.
package config

import (
	"fmt"
	"net/url"
	"os"
	"path/filepath"
	"strings"

	"gopkg.in/yaml.v3"
)

// Auth holds stored credentials for one host. SSO (authorization_code) logins
// set RefreshToken; client_credentials logins set ClientID/ClientSecret so the
// CLI can re-mint a token on expiry.
type Auth struct {
	Method       string `yaml:"method"` // "sso" | "client_credentials"
	AccessToken  string `yaml:"access_token"`
	RefreshToken string `yaml:"refresh_token,omitempty"`
	TokenType    string `yaml:"token_type,omitempty"`
	ExpiresAt    int64  `yaml:"expires_at,omitempty"` // unix seconds; 0 = unknown
	Scope        string `yaml:"scope,omitempty"`
	ClientID     string `yaml:"client_id,omitempty"`
	ClientSecret string `yaml:"client_secret,omitempty"`
}

// Host is one Campbooks deployment the CLI knows about.
type Host struct {
	Endpoint string `yaml:"endpoint"` // base URL, e.g. https://app.example.com
	Auth     *Auth  `yaml:"auth,omitempty"`
}

// Config is the whole on-disk state.
type Config struct {
	DefaultHost string           `yaml:"default_host,omitempty"`
	Hosts       map[string]*Host `yaml:"hosts,omitempty"`

	path string `yaml:"-"`
}

// Dir is the config directory (honors $CAMPBOOKS_CONFIG_DIR, then $XDG_CONFIG_HOME).
func Dir() string {
	if d := os.Getenv("CAMPBOOKS_CONFIG_DIR"); d != "" {
		return d
	}
	if xdg := os.Getenv("XDG_CONFIG_HOME"); xdg != "" {
		return filepath.Join(xdg, "campbooks")
	}
	home, _ := os.UserHomeDir()
	return filepath.Join(home, ".config", "campbooks")
}

func filePath() string { return filepath.Join(Dir(), "config.yml") }

// Load reads the config file, returning an empty (but usable) Config if none exists.
func Load() (*Config, error) {
	p := filePath()
	c := &Config{Hosts: map[string]*Host{}, path: p}

	data, err := os.ReadFile(p)
	if os.IsNotExist(err) {
		return c, nil
	}
	if err != nil {
		return nil, fmt.Errorf("reading %s: %w", p, err)
	}
	if err := yaml.Unmarshal(data, c); err != nil {
		return nil, fmt.Errorf("parsing %s: %w", p, err)
	}
	if c.Hosts == nil {
		c.Hosts = map[string]*Host{}
	}
	c.path = p
	return c, nil
}

// Save writes the config file with 0600 permissions, creating the dir as needed.
func (c *Config) Save() error {
	if err := os.MkdirAll(Dir(), 0o700); err != nil {
		return fmt.Errorf("creating config dir: %w", err)
	}
	data, err := yaml.Marshal(c)
	if err != nil {
		return err
	}
	// Write to a temp file then rename, so a crash never leaves a half-written
	// (secret-bearing) config.
	tmp := c.path + ".tmp"
	if err := os.WriteFile(tmp, data, 0o600); err != nil {
		return fmt.Errorf("writing config: %w", err)
	}
	return os.Rename(tmp, c.path)
}

// HostKey normalizes a base URL to its host[:port] key.
func HostKey(endpoint string) (string, error) {
	u, err := url.Parse(endpoint)
	if err != nil {
		return "", err
	}
	if u.Host == "" {
		return "", fmt.Errorf("missing host in %q", endpoint)
	}
	return u.Host, nil
}

// NormalizeEndpoint trims trailing slashes and defaults a bare host[:port] to a
// scheme: https for everything except loopback, which gets http.
func NormalizeEndpoint(raw string) (string, error) {
	raw = strings.TrimSpace(raw)
	if raw == "" {
		return "", fmt.Errorf("endpoint is empty")
	}
	if !strings.Contains(raw, "://") {
		if isLoopbackHost(raw) {
			raw = "http://" + raw
		} else {
			raw = "https://" + raw
		}
	}
	u, err := url.Parse(raw)
	if err != nil {
		return "", err
	}
	if u.Host == "" {
		return "", fmt.Errorf("invalid endpoint %q", raw)
	}
	u.Path = strings.TrimRight(u.Path, "/")
	u.RawQuery, u.Fragment = "", ""
	return u.String(), nil
}

func isLoopbackHost(hostport string) bool {
	h := hostport
	if i := strings.LastIndex(h, ":"); i != -1 {
		h = h[:i]
	}
	return h == "localhost" || h == "127.0.0.1" || h == "::1"
}

// UpsertHost records (or updates) a host's endpoint and returns it.
func (c *Config) UpsertHost(endpoint string) (string, *Host, error) {
	endpoint, err := NormalizeEndpoint(endpoint)
	if err != nil {
		return "", nil, err
	}
	key, err := HostKey(endpoint)
	if err != nil {
		return "", nil, err
	}
	h := c.Hosts[key]
	if h == nil {
		h = &Host{}
		c.Hosts[key] = h
	}
	h.Endpoint = endpoint
	if c.DefaultHost == "" {
		c.DefaultHost = key
	}
	return key, h, nil
}

// Resolve picks the host to act on: an explicit name/endpoint wins, else the
// default host. Returns the host key and record, or an error if none is usable.
func (c *Config) Resolve(explicit string) (string, *Host, error) {
	if explicit != "" {
		key := explicit
		if strings.Contains(explicit, "://") {
			var err error
			if key, err = HostKey(explicit); err != nil {
				return "", nil, err
			}
		}
		if h, ok := c.Hosts[key]; ok {
			return key, h, nil
		}
		return "", nil, fmt.Errorf("unknown host %q — run `campbooks login --host %s` first", explicit, explicit)
	}
	if c.DefaultHost == "" {
		return "", nil, fmt.Errorf("no host configured — run `campbooks login` first")
	}
	h, ok := c.Hosts[c.DefaultHost]
	if !ok {
		return "", nil, fmt.Errorf("default host %q is missing from the config", c.DefaultHost)
	}
	return c.DefaultHost, h, nil
}

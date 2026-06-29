// Package cmd wires the campbooks CLI's command tree (cobra).
package cmd

import (
	"context"
	"encoding/json"
	"fmt"
	"os"

	"github.com/notacamp/campbooks/cli/internal/client"
	"github.com/notacamp/campbooks/cli/internal/config"
	"github.com/spf13/cobra"
)

const defaultCloudHost = "app.campbooks.not-a-camp.com"

var (
	flagHost string
	flagJSON bool
	version  = "dev"
)

// Execute runs the CLI. v is the build version string.
func Execute(v string) {
	version = v
	if err := newRootCmd().ExecuteContext(context.Background()); err != nil {
		fmt.Fprintln(os.Stderr, "error:", err)
		os.Exit(1)
	}
}

func newRootCmd() *cobra.Command {
	root := &cobra.Command{
		Use:   "campbooks",
		Short: "Drive your Campbooks inbox, documents, and Scout from the terminal",
		Long: "campbooks is the developer CLI for Campbooks — Not A Camp's AI-native\n" +
			"email client. Sign in with `campbooks login`, then read and send email,\n" +
			"manage documents, contacts, and tags, and talk to Scout, all over the\n" +
			"public REST API.",
		SilenceUsage:  true,
		SilenceErrors: true,
		Version:       version,
	}
	root.PersistentFlags().StringVar(&flagHost, "host", "", "Campbooks host to act on (default: the configured host)")
	root.PersistentFlags().BoolVar(&flagJSON, "json", false, "output raw JSON instead of a table")

	root.AddCommand(
		newLoginCmd(),
		newLogoutCmd(),
		newWhoamiCmd(),
		newConfigCmd(),
		newEmailsCmd(),
		newDocumentsCmd(),
		newContactsCmd(),
		newTagsCmd(),
		newDocTypesCmd(),
		newScoutCmd(),
	)
	return root
}

// session bundles a resolved host with an authenticated client whose refreshed
// tokens are persisted back to the config.
type session struct {
	cfg     *config.Config
	hostKey string
	host    *config.Host
	client  *client.Client
}

// requireSession loads config, resolves the target host, and returns an
// authenticated client. It errors clearly if the user isn't logged in.
func requireSession() (*session, error) {
	cfg, err := config.Load()
	if err != nil {
		return nil, err
	}
	key, host, err := cfg.Resolve(flagHost)
	if err != nil {
		return nil, err
	}
	if host.Auth == nil || host.Auth.AccessToken == "" {
		return nil, fmt.Errorf("not signed in to %s — run `campbooks login`", key)
	}
	s := &session{cfg: cfg, hostKey: key, host: host}
	s.client = client.New(host.Endpoint, host.Auth, func(a *config.Auth) error {
		host.Auth = a
		return cfg.Save()
	})
	return s, nil
}

// Identity is the GET /api/v1/me payload.
type Identity struct {
	User struct {
		ID    string `json:"id"`
		Name  string `json:"name"`
		Email string `json:"email"`
	} `json:"user"`
	Workspace struct {
		ID   string `json:"id"`
		Name string `json:"name"`
	} `json:"workspace"`
	Scopes []string `json:"scopes"`
}

func fetchIdentity(ctx context.Context, c *client.Client) (*Identity, error) {
	data, _, err := c.Get(ctx, "/api/v1/me", nil)
	if err != nil {
		return nil, err
	}
	var id Identity
	if err := json.Unmarshal(data, &id); err != nil {
		return nil, err
	}
	return &id, nil
}

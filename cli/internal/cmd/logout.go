package cmd

import (
	"fmt"
	"net/http"
	"time"

	"github.com/notacamp/campbooks/cli/internal/auth"
	"github.com/notacamp/campbooks/cli/internal/config"
	"github.com/spf13/cobra"
)

func newLogoutCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "logout",
		Short: "Sign out of a Campbooks host",
		Args:  cobra.NoArgs,
		RunE: func(cmd *cobra.Command, _ []string) error {
			cfg, err := config.Load()
			if err != nil {
				return err
			}
			key, host, err := cfg.Resolve(flagHost)
			if err != nil {
				return err
			}
			if host.Auth == nil {
				fmt.Fprintf(cmd.OutOrStdout(), "Already signed out of %s\n", key)
				return nil
			}
			hc := &http.Client{Timeout: 30 * time.Second}
			_ = auth.Revoke(cmd.Context(), hc, host.Endpoint, host.Auth) // best effort
			host.Auth = nil
			if cfg.DefaultHost == key {
				cfg.DefaultHost = pickDefault(cfg, key)
			}
			if err := cfg.Save(); err != nil {
				return err
			}
			fmt.Fprintf(cmd.OutOrStdout(), "✓ Signed out of %s\n", key)
			return nil
		},
	}
}

// pickDefault chooses a remaining signed-in host to become the default.
func pickDefault(cfg *config.Config, exclude string) string {
	for k, h := range cfg.Hosts {
		if k != exclude && h.Auth != nil {
			return k
		}
	}
	return ""
}

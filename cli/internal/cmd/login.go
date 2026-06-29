package cmd

import (
	"bufio"
	"fmt"
	"net/http"
	"strings"
	"time"

	"github.com/notacamp/campbooks/cli/internal/auth"
	"github.com/notacamp/campbooks/cli/internal/client"
	"github.com/notacamp/campbooks/cli/internal/config"
	"github.com/spf13/cobra"
)

func newLoginCmd() *cobra.Command {
	var (
		noBrowser    bool
		clientID     string
		clientSecret string
		scopes       []string
	)
	cmd := &cobra.Command{
		Use:   "login",
		Short: "Sign in to a Campbooks host",
		Long: "Sign in to a Campbooks host and store the credentials.\n\n" +
			"By default this opens your browser (OAuth authorization_code + PKCE) and\n" +
			"you sign in with your normal Campbooks session. For CI/headless use, pass\n" +
			"--client-id/--client-secret (a client_credentials client from Settings →\n" +
			"API access), or --no-browser to paste a code manually.",
		Args: cobra.NoArgs,
		RunE: func(cmd *cobra.Command, _ []string) error {
			ctx := cmd.Context()
			cfg, err := config.Load()
			if err != nil {
				return err
			}

			endpoint := flagHost
			if endpoint == "" {
				if endpoint, err = promptEndpoint(cmd); err != nil {
					return err
				}
			}
			key, host, err := cfg.UpsertHost(endpoint)
			if err != nil {
				return err
			}

			hc := &http.Client{Timeout: 2 * time.Minute}
			var creds *config.Auth
			switch {
			case clientID != "" || clientSecret != "":
				if clientID == "" || clientSecret == "" {
					return fmt.Errorf("both --client-id and --client-secret are required for headless login")
				}
				creds, err = auth.ClientCredentialsLogin(ctx, hc, host.Endpoint, clientID, clientSecret, scopes)
			case noBrowser:
				creds, err = auth.OOBLogin(ctx, hc, host.Endpoint, scopes, cmd.InOrStdin(), cmd.OutOrStdout())
			default:
				creds, err = auth.BrowserLogin(ctx, hc, host.Endpoint, scopes, cmd.ErrOrStderr(), nil)
			}
			if err != nil {
				return err
			}

			host.Auth = creds
			cfg.DefaultHost = key
			if err := cfg.Save(); err != nil {
				return err
			}

			out := cmd.OutOrStdout()
			c := client.New(host.Endpoint, host.Auth, nil)
			if id, ierr := fetchIdentity(ctx, c); ierr == nil {
				fmt.Fprintf(out, "✓ Signed in to %s as %s (%s)\n", key, id.User.Email, id.Workspace.Name)
			} else {
				fmt.Fprintf(out, "✓ Signed in to %s\n", key)
			}
			return nil
		},
	}
	cmd.Flags().BoolVar(&noBrowser, "no-browser", false, "print a URL and paste the code instead of opening a browser")
	cmd.Flags().StringVar(&clientID, "client-id", "", "client_credentials client ID (headless/CI)")
	cmd.Flags().StringVar(&clientSecret, "client-secret", "", "client_credentials client secret (headless/CI)")
	cmd.Flags().StringSliceVar(&scopes, "scope", auth.DefaultScopes, "scopes to request")
	return cmd
}

func promptEndpoint(cmd *cobra.Command) (string, error) {
	out := cmd.OutOrStdout()
	fmt.Fprintf(out, "Campbooks host [%s]: ", defaultCloudHost)
	sc := bufio.NewScanner(cmd.InOrStdin())
	if !sc.Scan() {
		return "", fmt.Errorf("no host entered")
	}
	v := strings.TrimSpace(sc.Text())
	if v == "" {
		v = defaultCloudHost
	}
	return v, nil
}

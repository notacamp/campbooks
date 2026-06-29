package cmd

import (
	"fmt"
	"strings"

	"github.com/notacamp/campbooks/cli/internal/output"
	"github.com/spf13/cobra"
)

func newWhoamiCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "whoami",
		Short: "Show the signed-in user, workspace, and host",
		Args:  cobra.NoArgs,
		RunE: func(cmd *cobra.Command, _ []string) error {
			s, err := requireSession()
			if err != nil {
				return err
			}
			id, err := fetchIdentity(cmd.Context(), s.client)
			if err != nil {
				return err
			}
			out := cmd.OutOrStdout()
			if flagJSON {
				return output.JSON(out, map[string]any{
					"host":        s.hostKey,
					"endpoint":    s.host.Endpoint,
					"auth_method": s.host.Auth.Method,
					"user":        id.User,
					"workspace":   id.Workspace,
					"scopes":      id.Scopes,
				})
			}
			fmt.Fprintf(out, "User:      %s <%s>\n", id.User.Name, id.User.Email)
			fmt.Fprintf(out, "Workspace: %s\n", id.Workspace.Name)
			fmt.Fprintf(out, "Host:      %s (%s)\n", s.hostKey, s.host.Endpoint)
			fmt.Fprintf(out, "Auth:      %s\n", s.host.Auth.Method)
			if len(id.Scopes) > 0 {
				fmt.Fprintf(out, "Scopes:    %s\n", strings.Join(id.Scopes, ", "))
			}
			return nil
		},
	}
}

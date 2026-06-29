package cmd

import (
	"fmt"
	"sort"

	"github.com/notacamp/campbooks/cli/internal/config"
	"github.com/notacamp/campbooks/cli/internal/output"
	"github.com/spf13/cobra"
)

func newConfigCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "config",
		Short: "Manage CLI configuration and hosts",
	}
	cmd.AddCommand(newConfigListCmd(), newConfigUseCmd(), newConfigPathCmd())
	return cmd
}

func newConfigListCmd() *cobra.Command {
	return &cobra.Command{
		Use:     "list",
		Aliases: []string{"ls"},
		Short:   "List configured hosts",
		Args:    cobra.NoArgs,
		RunE: func(cmd *cobra.Command, _ []string) error {
			cfg, err := config.Load()
			if err != nil {
				return err
			}
			out := cmd.OutOrStdout()
			if len(cfg.Hosts) == 0 {
				fmt.Fprintln(out, "No hosts configured. Run `campbooks login`.")
				return nil
			}
			keys := make([]string, 0, len(cfg.Hosts))
			for k := range cfg.Hosts {
				keys = append(keys, k)
			}
			sort.Strings(keys)

			if flagJSON {
				return output.JSON(out, cfg)
			}
			t := output.NewTable(out, "host", "endpoint", "signed in", "default")
			for _, k := range keys {
				h := cfg.Hosts[k]
				signedIn := "no"
				if h.Auth != nil && h.Auth.AccessToken != "" {
					signedIn = h.Auth.Method
				}
				def := ""
				if k == cfg.DefaultHost {
					def = "✓"
				}
				t.Row(k, h.Endpoint, signedIn, def)
			}
			t.Flush()
			return nil
		},
	}
}

func newConfigUseCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "use <host>",
		Short: "Set the default host",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			cfg, err := config.Load()
			if err != nil {
				return err
			}
			key := args[0]
			if _, ok := cfg.Hosts[key]; !ok {
				return fmt.Errorf("unknown host %q — run `campbooks config list`", key)
			}
			cfg.DefaultHost = key
			if err := cfg.Save(); err != nil {
				return err
			}
			fmt.Fprintf(cmd.OutOrStdout(), "✓ Default host set to %s\n", key)
			return nil
		},
	}
}

func newConfigPathCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "path",
		Short: "Print the config file path",
		Args:  cobra.NoArgs,
		RunE: func(cmd *cobra.Command, _ []string) error {
			fmt.Fprintln(cmd.OutOrStdout(), config.Dir()+"/config.yml")
			return nil
		},
	}
}

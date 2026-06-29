package cmd

import (
	"github.com/spf13/cobra"
)

func newDocTypesCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:     "doctypes",
		Aliases: []string{"document-types"},
		Short:   "List the workspace's document types",
	}
	cmd.AddCommand(docTypesListCmd())
	return cmd
}

func docTypesListCmd() *cobra.Command {
	return &cobra.Command{
		Use:     "list",
		Aliases: []string{"ls"},
		Short:   "List document types",
		Args:    cobra.NoArgs,
		RunE: func(cmd *cobra.Command, _ []string) error {
			s, err := requireSession()
			if err != nil {
				return err
			}
			// Unpaginated endpoint — meta is nil.
			data, meta, err := s.client.Get(cmd.Context(), "/api/v1/document_types", nil)
			if err != nil {
				return err
			}
			return printList(cmd.OutOrStdout(), data, meta, []column{
				col("id", "id"),
				col("name", "name"),
				col("category", "category"),
				col("color", "color"),
				col("auto_star", "auto_star"),
			})
		},
	}
}

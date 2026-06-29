package cmd

import (
	"net/url"
	"strconv"

	"github.com/spf13/cobra"
)

func newTagsCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "tags",
		Short: "List the workspace's email tags",
	}
	cmd.AddCommand(tagsListCmd())
	return cmd
}

func tagsListCmd() *cobra.Command {
	var page, perPage int
	cmd := &cobra.Command{
		Use:     "list",
		Aliases: []string{"ls"},
		Short:   "List tags",
		Args:    cobra.NoArgs,
		RunE: func(cmd *cobra.Command, _ []string) error {
			s, err := requireSession()
			if err != nil {
				return err
			}
			q := url.Values{}
			if page > 0 {
				q.Set("page", strconv.Itoa(page))
			}
			if perPage > 0 {
				q.Set("per_page", strconv.Itoa(perPage))
			}
			data, meta, err := s.client.Get(cmd.Context(), "/api/v1/tags", q)
			if err != nil {
				return err
			}
			return printList(cmd.OutOrStdout(), data, meta, []column{
				col("id", "id"),
				col("name", "name"),
				col("color", "color"),
				col("group", "group_name"),
				col("source", "source"),
				col("account", "email_account_id"),
			})
		},
	}
	cmd.Flags().IntVar(&page, "page", 0, "page number")
	cmd.Flags().IntVar(&perPage, "per-page", 0, "results per page (max 100)")
	return cmd
}

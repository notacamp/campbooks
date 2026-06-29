package cmd

import (
	"fmt"
	"net/http"
	"net/url"
	"strconv"

	"github.com/spf13/cobra"
)

func newContactsCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:     "contacts",
		Aliases: []string{"contact"},
		Short:   "List and manage contacts",
	}
	cmd.AddCommand(contactsListCmd(), contactsGetCmd(), contactsUpdateCmd())
	for _, st := range []struct{ use, state, done string }{
		{"star", "star", "Starred"},
		{"unstar", "unstar", "Unstarred"},
		{"allow", "allow", "Allowed"},
		{"block", "block", "Blocked"},
		{"unblock", "unblock", "Unblocked"},
	} {
		cmd.AddCommand(contactsStateCmd(st.use, st.state, st.done))
	}
	return cmd
}

func contactsListCmd() *cobra.Command {
	var (
		listStatus    string
		starred       bool
		q             string
		page, perPage int
	)
	cmd := &cobra.Command{
		Use:     "list",
		Aliases: []string{"ls"},
		Short:   "List contacts",
		Args:    cobra.NoArgs,
		RunE: func(cmd *cobra.Command, _ []string) error {
			s, err := requireSession()
			if err != nil {
				return err
			}
			qv := url.Values{}
			setIf(qv, "list_status", listStatus)
			if cmd.Flags().Changed("starred") {
				qv.Set("starred", strconv.FormatBool(starred))
			}
			setIf(qv, "q", q)
			if page > 0 {
				qv.Set("page", strconv.Itoa(page))
			}
			if perPage > 0 {
				qv.Set("per_page", strconv.Itoa(perPage))
			}
			data, meta, err := s.client.Get(cmd.Context(), "/api/v1/contacts", qv)
			if err != nil {
				return err
			}
			return printList(cmd.OutOrStdout(), data, meta, []column{
				col("id", "id"),
				colTrunc("name", "name", 24),
				colTrunc("email", "email", 30),
				colTrunc("organization", "organization", 22),
				col("relationship", "relationship_type"),
				col("list", "list_status"),
				col("starred", "starred"),
			})
		},
	}
	f := cmd.Flags()
	f.StringVar(&listStatus, "list-status", "", "neutral|allowed|blocked")
	f.BoolVar(&starred, "starred", false, "only starred contacts")
	f.StringVarP(&q, "query", "q", "", "search name/email")
	f.IntVar(&page, "page", 0, "page number")
	f.IntVar(&perPage, "per-page", 0, "results per page (max 100)")
	return cmd
}

func contactsGetCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "get <id>",
		Short: "Show one contact",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			s, err := requireSession()
			if err != nil {
				return err
			}
			data, _, err := s.client.Get(cmd.Context(), "/api/v1/contacts/"+args[0], nil)
			if err != nil {
				return err
			}
			return printObject(cmd.OutOrStdout(), data)
		},
	}
}

func contactsUpdateCmd() *cobra.Command {
	var name, relationship string
	cmd := &cobra.Command{
		Use:   "update <id>",
		Short: "Update a contact's name or relationship type",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			s, err := requireSession()
			if err != nil {
				return err
			}
			form := url.Values{}
			setIf(form, "name", name)
			setIf(form, "relationship_type", relationship)
			if len(form) == 0 {
				return fmt.Errorf("pass --name and/or --relationship-type")
			}
			data, err := s.client.Send(cmd.Context(), http.MethodPatch, "/api/v1/contacts/"+args[0], nil, form)
			if err != nil {
				return err
			}
			return printResult(cmd.OutOrStdout(), data, "Updated")
		},
	}
	cmd.Flags().StringVar(&name, "name", "", "contact name")
	cmd.Flags().StringVar(&relationship, "relationship-type", "", "self|client|vendor|partner|service_provider|colleague|personal|unknown")
	return cmd
}

func contactsStateCmd(use, state, done string) *cobra.Command {
	return &cobra.Command{
		Use:   use + " <id>",
		Short: "Set contact state: " + state,
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			s, err := requireSession()
			if err != nil {
				return err
			}
			form := url.Values{"state": {state}}
			data, err := s.client.Send(cmd.Context(), http.MethodPost, "/api/v1/contacts/"+args[0]+"/state", nil, form)
			if err != nil {
				return err
			}
			return printResult(cmd.OutOrStdout(), data, done)
		},
	}
}

package cmd

import (
	"fmt"
	"net/http"
	"net/url"
	"strconv"

	"github.com/spf13/cobra"
)

func newEmailsCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:     "emails",
		Aliases: []string{"email"},
		Short:   "List, read, send, and tag email",
	}
	cmd.AddCommand(
		emailsListCmd(),
		emailsGetCmd(),
		emailsSendCmd(),
		emailsReplyCmd(),
		emailsMarkCmd("read", "mark_read", "Mark an email as read"),
		emailsMarkCmd("unread", "mark_unread", "Mark an email as unread"),
		emailsTagCmd(),
		emailsUntagCmd(),
	)
	return cmd
}

func emailsListCmd() *cobra.Command {
	var (
		unread, hasAttachment bool
		accounts              []int
		category, priority, q string
		after, before         string
		page, perPage         int
	)
	cmd := &cobra.Command{
		Use:     "list",
		Aliases: []string{"ls"},
		Short:   "List emails",
		Args:    cobra.NoArgs,
		RunE: func(cmd *cobra.Command, _ []string) error {
			s, err := requireSession()
			if err != nil {
				return err
			}
			q2 := url.Values{}
			if cmd.Flags().Changed("unread") {
				q2.Set("unread", strconv.FormatBool(unread))
			}
			if cmd.Flags().Changed("has-attachment") {
				q2.Set("has_attachment", strconv.FormatBool(hasAttachment))
			}
			for _, a := range accounts {
				q2.Add("account_ids[]", strconv.Itoa(a))
			}
			setIf(q2, "category", category)
			setIf(q2, "priority", priority)
			setIf(q2, "q", q)
			setIf(q2, "received_after", after)
			setIf(q2, "received_before", before)
			if page > 0 {
				q2.Set("page", strconv.Itoa(page))
			}
			if perPage > 0 {
				q2.Set("per_page", strconv.Itoa(perPage))
			}

			data, meta, err := s.client.Get(cmd.Context(), "/api/v1/emails", q2)
			if err != nil {
				return err
			}
			return printList(cmd.OutOrStdout(), data, meta, []column{
				col("id", "id"),
				col("read", "read"),
				colTrunc("from", "from", 28),
				colTrunc("subject", "subject", 50),
				col("category", "category"),
				col("received", "received_at"),
			})
		},
	}
	f := cmd.Flags()
	f.BoolVar(&unread, "unread", false, "only unread emails")
	f.BoolVar(&hasAttachment, "has-attachment", false, "only emails with attachments")
	f.IntSliceVar(&accounts, "account", nil, "restrict to email account ID(s)")
	f.StringVar(&category, "category", "", "filter by category")
	f.StringVar(&priority, "priority", "", "filter by priority (low|medium|high)")
	f.StringVarP(&q, "query", "q", "", "search subject/sender")
	f.StringVar(&after, "received-after", "", "only emails received after (ISO 8601)")
	f.StringVar(&before, "received-before", "", "only emails received before (ISO 8601)")
	f.IntVar(&page, "page", 0, "page number")
	f.IntVar(&perPage, "per-page", 0, "results per page (max 100)")
	return cmd
}

func emailsGetCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "get <id>",
		Short: "Show one email, including its body",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			s, err := requireSession()
			if err != nil {
				return err
			}
			data, _, err := s.client.Get(cmd.Context(), "/api/v1/emails/"+args[0], nil)
			if err != nil {
				return err
			}
			return printObject(cmd.OutOrStdout(), data)
		},
	}
}

func emailsSendCmd() *cobra.Command {
	var (
		account                     int
		to, subject, body, bodyFile string
		cc, bcc                     string
	)
	cmd := &cobra.Command{
		Use:   "send",
		Short: "Send a new email",
		Args:  cobra.NoArgs,
		RunE: func(cmd *cobra.Command, _ []string) error {
			s, err := requireSession()
			if err != nil {
				return err
			}
			if account == 0 || to == "" {
				return fmt.Errorf("--account and --to are required")
			}
			b, err := readBody(cmd, body, bodyFile)
			if err != nil {
				return err
			}
			form := url.Values{}
			form.Set("email_account_id", strconv.Itoa(account))
			form.Set("to_address", to)
			setIf(form, "subject", subject)
			setIf(form, "body", b)
			setIf(form, "cc_address", cc)
			setIf(form, "bcc_address", bcc)
			data, err := s.client.Send(cmd.Context(), http.MethodPost, "/api/v1/emails", nil, form)
			if err != nil {
				return err
			}
			return printResult(cmd.OutOrStdout(), data, "Sent")
		},
	}
	f := cmd.Flags()
	f.IntVar(&account, "account", 0, "email account ID to send from (required)")
	f.StringVar(&to, "to", "", "recipient address (required)")
	f.StringVar(&subject, "subject", "", "subject")
	f.StringVar(&body, "body", "", "body (HTML or text)")
	f.StringVar(&bodyFile, "body-file", "", "read the body from a file (- for stdin)")
	f.StringVar(&cc, "cc", "", "cc address")
	f.StringVar(&bcc, "bcc", "", "bcc address")
	return cmd
}

func emailsReplyCmd() *cobra.Command {
	var (
		account        int
		body, bodyFile string
		to, cc, bcc    string
	)
	cmd := &cobra.Command{
		Use:   "reply <id>",
		Short: "Reply to an email (threads automatically)",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			s, err := requireSession()
			if err != nil {
				return err
			}
			b, err := readBody(cmd, body, bodyFile)
			if err != nil {
				return err
			}
			if b == "" {
				return fmt.Errorf("--body (or --body-file) is required")
			}
			form := url.Values{}
			form.Set("body", b)
			if account > 0 {
				form.Set("email_account_id", strconv.Itoa(account))
			}
			setIf(form, "to_address", to)
			setIf(form, "cc_address", cc)
			setIf(form, "bcc_address", bcc)
			data, err := s.client.Send(cmd.Context(), http.MethodPost, "/api/v1/emails/"+args[0]+"/reply", nil, form)
			if err != nil {
				return err
			}
			return printResult(cmd.OutOrStdout(), data, "Replied")
		},
	}
	f := cmd.Flags()
	f.StringVar(&body, "body", "", "reply body (required)")
	f.StringVar(&bodyFile, "body-file", "", "read the body from a file (- for stdin)")
	f.IntVar(&account, "account", 0, "email account ID to send from (defaults to the source account)")
	f.StringVar(&to, "to", "", "recipient (defaults to the original sender)")
	f.StringVar(&cc, "cc", "", "cc address")
	f.StringVar(&bcc, "bcc", "", "bcc address")
	return cmd
}

func emailsMarkCmd(use, action, short string) *cobra.Command {
	return &cobra.Command{
		Use:   use + " <id>",
		Short: short,
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			s, err := requireSession()
			if err != nil {
				return err
			}
			data, err := s.client.Send(cmd.Context(), http.MethodPost, "/api/v1/emails/"+args[0]+"/"+action, nil, url.Values{})
			if err != nil {
				return err
			}
			return printResult(cmd.OutOrStdout(), data, "Updated")
		},
	}
}

func emailsTagCmd() *cobra.Command {
	var tagID int
	cmd := &cobra.Command{
		Use:   "tag <email-id> [tag-name]",
		Short: "Add an existing workspace tag to an email",
		Args:  cobra.RangeArgs(1, 2),
		RunE: func(cmd *cobra.Command, args []string) error {
			s, err := requireSession()
			if err != nil {
				return err
			}
			form := url.Values{}
			switch {
			case tagID > 0:
				form.Set("tag_id", strconv.Itoa(tagID))
			case len(args) == 2:
				form.Set("name", args[1])
			default:
				return fmt.Errorf("provide a tag name or --tag-id")
			}
			data, err := s.client.Send(cmd.Context(), http.MethodPost, "/api/v1/emails/"+args[0]+"/tags", nil, form)
			if err != nil {
				return err
			}
			return printResult(cmd.OutOrStdout(), data, "Tagged")
		},
	}
	cmd.Flags().IntVar(&tagID, "tag-id", 0, "tag ID (instead of a name)")
	return cmd
}

func emailsUntagCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "untag <email-id> <tag-id>",
		Short: "Remove a tag from an email",
		Args:  cobra.ExactArgs(2),
		RunE: func(cmd *cobra.Command, args []string) error {
			s, err := requireSession()
			if err != nil {
				return err
			}
			_, err = s.client.Send(cmd.Context(), http.MethodDelete, "/api/v1/emails/"+args[0]+"/tags/"+args[1], nil, nil)
			if err != nil {
				return err
			}
			return printResult(cmd.OutOrStdout(), nil, "Untagged")
		},
	}
}

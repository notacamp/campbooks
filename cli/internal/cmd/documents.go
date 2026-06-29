package cmd

import (
	"encoding/json"
	"fmt"
	"io"
	"mime"
	"net/http"
	"net/url"
	"os"
	"strconv"
	"strings"

	"github.com/notacamp/campbooks/cli/internal/output"
	"github.com/spf13/cobra"
)

func newDocumentsCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:     "documents",
		Aliases: []string{"docs", "document"},
		Short:   "List, download, upload, and review documents",
	}
	cmd.AddCommand(
		documentsListCmd(),
		documentsGetCmd(),
		documentsDownloadCmd(),
		documentsUploadCmd(),
		documentsUpdateCmd(),
		documentsActionCmd("approve", "approve", "Approve a document under review", "Approved"),
		documentsActionCmd("reject", "reject", "Reject a document under review", "Rejected"),
		documentsReclassifyCmd(),
	)
	return cmd
}

func documentsListCmd() *cobra.Command {
	var (
		docType       int
		reviewStatus  string
		aiStatus      string
		page, perPage int
	)
	cmd := &cobra.Command{
		Use:     "list",
		Aliases: []string{"ls"},
		Short:   "List documents",
		Args:    cobra.NoArgs,
		RunE: func(cmd *cobra.Command, _ []string) error {
			s, err := requireSession()
			if err != nil {
				return err
			}
			q := url.Values{}
			if docType > 0 {
				q.Set("type", strconv.Itoa(docType))
			}
			setIf(q, "review_status", reviewStatus)
			setIf(q, "ai_status", aiStatus)
			if page > 0 {
				q.Set("page", strconv.Itoa(page))
			}
			if perPage > 0 {
				q.Set("per_page", strconv.Itoa(perPage))
			}
			data, meta, err := s.client.Get(cmd.Context(), "/api/v1/documents", q)
			if err != nil {
				return err
			}
			return printList(cmd.OutOrStdout(), data, meta, []column{
				col("id", "id"),
				colTrunc("title", "title", 40),
				colFn("type", func(m map[string]any) string { return cell(m["document_type"]) }),
				col("ai", "ai_status"),
				col("review", "review_status"),
				col("date", "document_date"),
			})
		},
	}
	f := cmd.Flags()
	f.IntVar(&docType, "type", 0, "filter by document type ID")
	f.StringVar(&reviewStatus, "review-status", "", "pending|approved|rejected")
	f.StringVar(&aiStatus, "ai-status", "", "pending|processing|completed|failed")
	f.IntVar(&page, "page", 0, "page number")
	f.IntVar(&perPage, "per-page", 0, "results per page (max 100)")
	return cmd
}

func documentsGetCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "get <id>",
		Short: "Show one document (with file info + extraction)",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			s, err := requireSession()
			if err != nil {
				return err
			}
			data, _, err := s.client.Get(cmd.Context(), "/api/v1/documents/"+args[0], nil)
			if err != nil {
				return err
			}
			return printObject(cmd.OutOrStdout(), data)
		},
	}
}

func documentsDownloadCmd() *cobra.Command {
	var out string
	cmd := &cobra.Command{
		Use:   "download <id>",
		Short: "Download a document's original file",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			s, err := requireSession()
			if err != nil {
				return err
			}
			resp, err := s.client.Download(cmd.Context(), "/api/v1/documents/"+args[0]+"/file")
			if err != nil {
				return err
			}
			defer resp.Body.Close()

			if out == "-" {
				_, err := io.Copy(cmd.OutOrStdout(), resp.Body)
				return err
			}
			dest := out
			if dest == "" {
				dest = filenameFromResp(resp)
			}
			if dest == "" {
				dest = "document-" + args[0]
			}
			f, err := os.Create(dest)
			if err != nil {
				return err
			}
			defer f.Close()
			if _, err := io.Copy(f, resp.Body); err != nil {
				return err
			}
			fmt.Fprintf(cmd.ErrOrStderr(), "✓ Saved %s\n", dest)
			return nil
		},
	}
	cmd.Flags().StringVarP(&out, "output", "o", "", "output file (default: the server filename; - for stdout)")
	return cmd
}

func documentsUploadCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "upload <file>...",
		Short: "Upload document file(s); AI classification runs asynchronously",
		Args:  cobra.MinimumNArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			s, err := requireSession()
			if err != nil {
				return err
			}
			data, err := s.client.Upload(cmd.Context(), "/api/v1/documents", "files[]", args, nil)
			if err != nil {
				return err
			}
			out := cmd.OutOrStdout()
			if flagJSON {
				return output.RawJSON(out, data)
			}
			var docs []map[string]any
			_ = json.Unmarshal(data, &docs)
			fmt.Fprintf(out, "✓ Uploaded %d file(s); AI processing is queued.\n", len(docs))
			for _, d := range docs {
				fmt.Fprintf(out, "  id %s  %s\n", cell(d["id"]), cell(d["canonical_filename"]))
			}
			return nil
		},
	}
}

func documentsUpdateCmd() *cobra.Command {
	var (
		vendorName, currency, documentDate, dueDate string
		invoiceNumber, description                  string
		amountCents, docType                        int
		sets                                        []string
	)
	cmd := &cobra.Command{
		Use:   "update <id>",
		Short: "Update extracted document fields (no review-state change)",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			s, err := requireSession()
			if err != nil {
				return err
			}
			form := url.Values{}
			setIf(form, "vendor_name", vendorName)
			setIf(form, "currency", currency)
			setIf(form, "document_date", documentDate)
			setIf(form, "due_date", dueDate)
			setIf(form, "invoice_number", invoiceNumber)
			setIf(form, "description", description)
			if cmd.Flags().Changed("amount-cents") {
				form.Set("amount_cents", strconv.Itoa(amountCents))
			}
			if docType > 0 {
				form.Set("document_type_id", strconv.Itoa(docType))
			}
			for _, kv := range sets {
				k, v, ok := strings.Cut(kv, "=")
				if !ok {
					return fmt.Errorf("--set expects key=value, got %q", kv)
				}
				form.Set(k, v)
			}
			if len(form) == 0 {
				return fmt.Errorf("nothing to update — pass at least one field (see --help) or --set field=value")
			}
			data, err := s.client.Send(cmd.Context(), http.MethodPatch, "/api/v1/documents/"+args[0], nil, form)
			if err != nil {
				return err
			}
			return printResult(cmd.OutOrStdout(), data, "Updated")
		},
	}
	f := cmd.Flags()
	f.StringVar(&vendorName, "vendor-name", "", "vendor name")
	f.IntVar(&amountCents, "amount-cents", 0, "amount in cents")
	f.StringVar(&currency, "currency", "", "ISO 4217 currency code")
	f.StringVar(&documentDate, "document-date", "", "document date (YYYY-MM-DD)")
	f.StringVar(&dueDate, "due-date", "", "due date (YYYY-MM-DD)")
	f.StringVar(&invoiceNumber, "invoice-number", "", "invoice number")
	f.StringVar(&description, "description", "", "description")
	f.IntVar(&docType, "type", 0, "document type ID")
	f.StringArrayVar(&sets, "set", nil, "set any other editable field: --set field=value (repeatable)")
	return cmd
}

func documentsActionCmd(use, action, short, done string) *cobra.Command {
	return &cobra.Command{
		Use:   use + " <id>",
		Short: short,
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			s, err := requireSession()
			if err != nil {
				return err
			}
			data, err := s.client.Send(cmd.Context(), http.MethodPost, "/api/v1/documents/"+args[0]+"/"+action, nil, url.Values{})
			if err != nil {
				return err
			}
			return printResult(cmd.OutOrStdout(), data, done)
		},
	}
}

func documentsReclassifyCmd() *cobra.Command {
	var docType int
	cmd := &cobra.Command{
		Use:   "reclassify <id>",
		Short: "Change a document's type (also marks it approved)",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			s, err := requireSession()
			if err != nil {
				return err
			}
			if docType == 0 {
				return fmt.Errorf("--type is required")
			}
			form := url.Values{"document_type_id": {strconv.Itoa(docType)}}
			data, err := s.client.Send(cmd.Context(), http.MethodPost, "/api/v1/documents/"+args[0]+"/reclassify", nil, form)
			if err != nil {
				return err
			}
			return printResult(cmd.OutOrStdout(), data, "Reclassified")
		},
	}
	cmd.Flags().IntVar(&docType, "type", 0, "new document type ID (required)")
	return cmd
}

func filenameFromResp(resp *http.Response) string {
	cd := resp.Header.Get("Content-Disposition")
	if cd == "" {
		return ""
	}
	if _, params, err := mime.ParseMediaType(cd); err == nil {
		return params["filename"]
	}
	return ""
}

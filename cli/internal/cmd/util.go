package cmd

import (
	"encoding/json"
	"fmt"
	"io"
	"net/url"
	"os"

	"github.com/notacamp/campbooks/cli/internal/output"
	"github.com/spf13/cobra"
)

// setIf sets k=val only when val is non-empty (keeps optional filters/fields out
// of the request unless the user provided them).
func setIf(v url.Values, k, val string) {
	if val != "" {
		v.Set(k, val)
	}
}

// intField reads a JSON number field (decoded as float64) as an int.
func intField(m map[string]any, key string) int {
	if f, ok := m[key].(float64); ok {
		return int(f)
	}
	return 0
}

// readBody resolves a command body from --body or --body-file ("-" = stdin).
func readBody(cmd *cobra.Command, body, bodyFile string) (string, error) {
	if bodyFile == "" {
		return body, nil
	}
	if bodyFile == "-" {
		b, err := io.ReadAll(cmd.InOrStdin())
		return string(b), err
	}
	b, err := os.ReadFile(bodyFile)
	return string(b), err
}

// printResult reports a successful write: the raw JSON with --json, otherwise a
// checkmark plus the created/updated record's id when present.
func printResult(out io.Writer, data json.RawMessage, msg string) error {
	if flagJSON {
		if len(data) == 0 {
			return nil
		}
		return output.RawJSON(out, data)
	}
	if len(data) > 0 {
		var m map[string]any
		if json.Unmarshal(data, &m) == nil {
			if id, ok := m["id"]; ok {
				fmt.Fprintf(out, "✓ %s (id %s)\n", msg, cell(id))
				return nil
			}
		}
	}
	fmt.Fprintf(out, "✓ %s\n", msg)
	return nil
}

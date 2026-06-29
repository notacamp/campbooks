package cmd

import (
	"encoding/json"
	"fmt"
	"io"
	"sort"
	"strconv"
	"strings"

	"github.com/notacamp/campbooks/cli/internal/client"
	"github.com/notacamp/campbooks/cli/internal/output"
)

// column is one table column: a header and a function pulling its cell from a
// decoded JSON row.
type column struct {
	header string
	get    func(map[string]any) string
}

// col maps a column header to a top-level JSON key.
func col(header, key string) column {
	return column{header: header, get: func(m map[string]any) string { return cell(m[key]) }}
}

// colTrunc is like col but truncates the cell to n runes.
func colTrunc(header, key string, n int) column {
	return column{header: header, get: func(m map[string]any) string { return output.Truncate(cell(m[key]), n) }}
}

// colFn builds a column from an arbitrary row function.
func colFn(header string, fn func(map[string]any) string) column {
	return column{header: header, get: fn}
}

// cell formats a decoded JSON value for display.
func cell(v any) string {
	switch x := v.(type) {
	case nil:
		return ""
	case string:
		return x
	case bool:
		if x {
			return "yes"
		}
		return "no"
	case float64:
		if x == float64(int64(x)) {
			return strconv.FormatInt(int64(x), 10)
		}
		return strconv.FormatFloat(x, 'f', -1, 64)
	case []any:
		parts := make([]string, len(x))
		for i, e := range x {
			parts[i] = cell(e)
		}
		return strings.Join(parts, ", ")
	case map[string]any:
		for _, k := range []string{"name", "title", "email"} {
			if s, ok := x[k].(string); ok {
				return s
			}
		}
		b, _ := json.Marshal(x)
		return string(b)
	default:
		return fmt.Sprintf("%v", x)
	}
}

func field(m map[string]any, key string) string { return cell(m[key]) }

// printList renders a JSON array as a table (or raw JSON with --json).
func printList(out io.Writer, data json.RawMessage, meta *client.Meta, cols []column) error {
	if flagJSON {
		return output.RawJSON(out, data)
	}
	var rows []map[string]any
	if err := json.Unmarshal(data, &rows); err != nil {
		return err
	}
	if len(rows) == 0 {
		fmt.Fprintln(out, "No results.")
		return nil
	}
	headers := make([]string, len(cols))
	for i, c := range cols {
		headers[i] = c.header
	}
	t := output.NewTable(out, headers...)
	for _, r := range rows {
		cells := make([]string, len(cols))
		for i, c := range cols {
			cells[i] = c.get(r)
		}
		t.Row(cells...)
	}
	t.Flush()
	if meta != nil && meta.TotalPages > 1 {
		fmt.Fprintf(out, "\nPage %d of %d · %d total (use --page)\n", meta.Page, meta.TotalPages, meta.Total)
	}
	return nil
}

// printObject renders a single JSON object as aligned key/value lines (or raw
// JSON with --json). Long values are truncated; use --json for the full value.
func printObject(out io.Writer, data json.RawMessage) error {
	if flagJSON {
		return output.RawJSON(out, data)
	}
	var m map[string]any
	if err := json.Unmarshal(data, &m); err != nil {
		return err
	}
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	t := output.NewTable(out, "field", "value")
	for _, k := range keys {
		t.Row(k, output.Truncate(cell(m[k]), 100))
	}
	t.Flush()
	return nil
}

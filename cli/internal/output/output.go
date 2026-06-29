// Package output renders command results as either an aligned table (default,
// for humans) or raw JSON (--json, for scripts). It respects NO_COLOR and only
// colorizes when writing to a terminal.
package output

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"strings"
	"text/tabwriter"
)

// JSON pretty-prints any value.
func JSON(w io.Writer, v any) error {
	enc := json.NewEncoder(w)
	enc.SetIndent("", "  ")
	enc.SetEscapeHTML(false)
	return enc.Encode(v)
}

// RawJSON pretty-prints already-encoded JSON bytes (re-indented). Falls back to
// the raw bytes if they don't parse.
func RawJSON(w io.Writer, raw []byte) error {
	if len(strings.TrimSpace(string(raw))) == 0 {
		return nil
	}
	var v any
	if err := json.Unmarshal(raw, &v); err != nil {
		_, werr := w.Write(raw)
		return werr
	}
	return JSON(w, v)
}

// Table is a simple tab-aligned table writer.
type Table struct {
	w       *tabwriter.Writer
	colored bool
	cols    int
}

// NewTable starts a table with the given column headers.
func NewTable(w io.Writer, headers ...string) *Table {
	tw := tabwriter.NewWriter(w, 0, 2, 2, ' ', 0)
	t := &Table{w: tw, colored: colorEnabled(w), cols: len(headers)}
	cells := make([]string, len(headers))
	for i, h := range headers {
		h = strings.ToUpper(h)
		if t.colored {
			h = bold(h)
		}
		cells[i] = h
	}
	fmt.Fprintln(tw, strings.Join(cells, "\t"))
	return t
}

// Row appends one row; cells are sanitized of tabs/newlines.
func (t *Table) Row(cells ...string) {
	out := make([]string, len(cells))
	for i, c := range cells {
		out[i] = sanitizeCell(c)
	}
	fmt.Fprintln(t.w, strings.Join(out, "\t"))
}

// Flush writes the table to the underlying writer.
func (t *Table) Flush() { _ = t.w.Flush() }

// Truncate shortens s to n runes with an ellipsis.
func Truncate(s string, n int) string {
	r := []rune(s)
	if len(r) <= n {
		return s
	}
	if n <= 1 {
		return string(r[:n])
	}
	return string(r[:n-1]) + "…"
}

func sanitizeCell(s string) string {
	s = strings.ReplaceAll(s, "\t", " ")
	s = strings.ReplaceAll(s, "\n", " ")
	s = strings.ReplaceAll(s, "\r", " ")
	if s == "" {
		return "-"
	}
	return s
}

func colorEnabled(w io.Writer) bool {
	if os.Getenv("NO_COLOR") != "" || os.Getenv("TERM") == "dumb" {
		return false
	}
	f, ok := w.(*os.File)
	if !ok {
		return false
	}
	info, err := f.Stat()
	if err != nil {
		return false
	}
	return info.Mode()&os.ModeCharDevice != 0
}

func bold(s string) string { return "\x1b[1m" + s + "\x1b[0m" }

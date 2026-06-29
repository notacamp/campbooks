package cmd

import "testing"

func TestCell(t *testing.T) {
	cases := []struct {
		in   any
		want string
	}{
		{nil, ""},
		{"hi", "hi"},
		{true, "yes"},
		{false, "no"},
		{float64(42), "42"},
		{float64(12.5), "12.5"},
		{[]any{"a", "b"}, "a, b"},
		{map[string]any{"name": "ACME"}, "ACME"},
		{map[string]any{"id": float64(3)}, `{"id":3}`},
	}
	for _, c := range cases {
		if got := cell(c.in); got != c.want {
			t.Errorf("cell(%v) = %q, want %q", c.in, got, c.want)
		}
	}
}

func TestIntField(t *testing.T) {
	m := map[string]any{"id": float64(7)}
	if got := intField(m, "id"); got != 7 {
		t.Errorf("intField id = %d", got)
	}
	if got := intField(m, "missing"); got != 0 {
		t.Errorf("intField missing = %d", got)
	}
}

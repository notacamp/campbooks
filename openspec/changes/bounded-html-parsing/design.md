## Context
html5ever 0.40.1 is the only HTML parser in the workspace: ammonia 4.2.3 and
css-inline 0.22 use it, and `sanitize::styles` already parses with it into a
small `Measure` tree sink. The tree builder's quadratic paths all go through
the sink: every scope check reads the name of each open element it passes
(`TreeSink::elem_name`), and foster parenting and the adoption agency move
nodes (`append_before_sibling`, `remove_from_parent`, `reparent_children`),
which ammonia's `RcDom` does by searching the parent's child list. Counting
those calls, weighted as `RcDom` pays for them, measures what every later
parse of the same input will cost, because they run the same algorithm on
the same tokens.

Measured with a standalone release build (counts are the sink's):

| input | size | parse | ammonia | `elem_name` calls |
|---|---|---|---|---|
| `<div></span>` × 5,000 | 58 KiB | 156 ms | 165 ms | 25 M |
| `<div></span>` × 175,000 | 2 MiB | 140 s | 181 s | 30,600 M |
| `<div>` × 200,000, unclosed | 976 KiB | 167 s | 86 s | 40,000 M |
| `<a><table><a>` × 50,000 | 683 KiB | 32 ms | 3.0 s | 1.3 M (+ moves) |
| newsletter, 60,000 elements | 1.5 MiB | 31 ms | 57 ms | 0.46 M |
| unclosed `<td>` tables × 5,000 | 166 KiB | 7 ms | 14 ms | 0.26 M |

## Decisions
- **A work budget, not a byte cap.** A byte cap low enough to stop the
  hostile cases (under ~50 KiB) would cut ordinary newsletters; the work
  separates them by two orders of magnitude.
- **Counted by the sink, stopped between chunks.** A parse cannot be
  cancelled from outside, and a timeout would leave the work running. The
  measure feeds the HTML 8 KiB at a time and checks the count after each
  chunk, so it stops within one chunk of the budget (a few tens of ms at
  worst) instead of finishing a parse that would take minutes.
- **Units.** Every sink call is 1; `elem_name` is 1 (it is the scope scan);
  `append_before_sibling` and `remove_from_parent` add the number of
  children of the parent they touch; `reparent_children` adds the number of
  children moved. Budget 5,000,000 (`MAX_PARSE_WORK`); ordinary mail stays
  under 1,000,000 at the 2 MiB read cap. Depth past 4,096
  (`MAX_PARSE_DEPTH`) is refused too, so no later recursive code sees a
  tree that deep; ordinary mail is under 100.
- **Shown as text, not refused.** The reader still shows the words, from
  the tokenizer (linear), escaped into one `<div style="white-space:
  pre-wrap">`. Nothing from the message's markup survives, so there is
  nothing to sanitise.
- **Search uses the tokenizer only.** It never needed a tree: it wants the
  text. The tokenizer is linear, decodes entities itself, and switches to raw
  text for `script`, `style`, `title` and the others where the tree builder
  would (as `styles::scan` already does).

## Risks
- A real message past the budget is shown as text. The margin over the
  largest ordinary mail measured is more than ten times.
- The measure adds one parse to messages without a style sheet: about 30 ms
  at the largest.

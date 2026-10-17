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
- **Counted by the sink, stopped between chunks.** A timeout around a
  later parse would leave the work running. The measure feeds the HTML
  1 KiB at a time and checks the count after each chunk, so most crafted
  bodies stop within a few tens of milliseconds instead of finishing a
  parse that would take minutes.
- **Both parse modes.** The inliner parses a whole document and ammonia a
  fragment in a `div` (its `make_parser`, default options), and the two can
  differ wholesale: after a leading `<frameset>` a document drops every
  token, while a fragment reads them all. The measure parses both ways, with
  exactly those options, and both must stay within the budget. The later
  parses then read serialisations of trees no bigger or deeper than the
  measured ones (css-inline only runs within 20,000 elements and 256 levels).
- **A time limit too.** Two reviews found work html5ever does without the
  sink, or before the count is read: the duplicate-attribute check inside one
  start tag (quadratic in its attributes, no sink call until `>`), the
  "Noah's ark" scan of the active formatting elements (attributes cloned and
  sorted per comparison), and stack walks within one chunk or at end of
  input. Counting cannot see those, so the measure also stops when both
  parses together pass 500 ms (`MAX_PARSE_TIME`, about three times what the
  densest ordinary 2 MiB mail needs on the production CPU): the sink reads
  the clock every 1,024 calls and unwinds out of the parse (a running parse
  cannot be stopped any other way; `resume_unwind` skips the panic hook, and
  other panics propagate), and the clock is read after every 1 KiB chunk,
  which bounds the tokenizer inside one long tag. Search's tokenizer pass is
  fed the same way and keeps the words read within 500 ms.
- **Units.** Every sink call is 1; `elem_name` is 1 (it is the scope scan);
  creating an element adds its attributes, and `add_attrs_if_missing` adds
  the element's attributes plus the new ones (ammonia's `RcDom` rebuilds a
  set of the existing names on every call: `<html a1><html a2>…` cost it
  5 s at 243 KiB); `append_before_sibling` and `remove_from_parent` add the
  number of children of the parent they touch; `reparent_children` adds the number of
  children moved. Budget 10,000,000 (`MAX_PARSE_WORK`): a dense newsletter
  of nested tables, links and images costs about 830,000 units at 1.25 MiB,
  so about 1,400,000 at the 2 MiB read cap; most mail is under 100 KiB and
  costs a few tens of thousands. With the PR's measure, a crafted 2 MiB
  message is refused in under 40 ms, and one crafted to stay just under the
  budget costs each later parse a few tens of milliseconds. Depth past 4,096
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
  densest ordinary mail at the read cap is about seven times.
- The measure adds one parse to messages without a style sheet: about 30 ms
  at the largest.

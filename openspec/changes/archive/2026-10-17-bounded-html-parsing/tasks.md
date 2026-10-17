# Tasks

## 1. Measure
- [x] 1.1 Release-build benchmark of html5ever and ammonia on hostile and ordinary HTML (design.md)

## 2. Server
- [x] 2.1 `sanitize::styles`: `measure(html) -> Option<Shape>`, chunked, counting work in the sink, `None` past `MAX_PARSE_WORK` or `MAX_PARSE_DEPTH`; `inline` takes the measured shape
- [x] 2.2 `sanitize`: measure first; past the budget, the text in a `pre-wrap` `div`, no images, no `css`
- [x] 2.3 `plain_text`: text from the tokenizer only (`text_of`), quoted `<blockquote>`s kept for the reader and dropped for search; `from_html` uses it
- [x] 2.4 Tests: hostile inputs stop at the budget; every fixture under `fixtures/email-html` passes it with room; the reader shows a hostile body as text; search text unchanged for the existing cases and linear on hostile input; a deep but cheap tree is sanitised on a 2 MiB-stack thread

## 3. Finish
- [x] 3.1 CHANGELOG (Security), recording, archive this change

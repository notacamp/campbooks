## Why

An HTML-only email over 256 KiB whose words `conversations.get` reads in
time is neither `text_unread` nor `truncated` (the reader shows its HTML
whole). A reply or forward then quoted only its first 256 KiB, and the
composer showed no "first part" note (campbooks-cloud#255, from the
re-review of #575). The reply dialog's preview was also cut at 280
characters without saying so.

## What Changes

- `MessageView` gains `quote_cut` (sent only when true). It means `text`
  holds only the first part of the message's words: the text part was cut,
  the HTML was cut at 256 KiB, or the words were cut at their limit. The
  reader's `truncated` stays about the text part, so the reader shows no
  "truncated" for HTML it shows whole.
- The composer shows its existing note ("This email is long: the quote
  holds its first part.") when the quoted message has `quote_cut`, as well
  as when its text or words were truncated.
- The reply dialog's preview says what it leaves out:
  - "This email is long: your reply quotes its first part." when the quote
    is cut.
  - Otherwise "Shown in part: your reply quotes all of it." when the
    preview shows only the start (it now ends with "…").
- The demo Gmail gains a long HTML email ("Tide tables for the season"),
  and the `quote-long-html` scenario records both notes.

## Impact

- Specs: conversations (A quote says when it is cut), inbox-ui (Saying what
  a quote leaves out).
- Code: `features/get_conversation`, `openapi.json` and client types,
  `ReplyComposer`, `ReplyDialog`, `ReplyQuote` (an optional `note`), i18n in
  en, pt-PT, es-ES and fr, `scripts/fake-gmail.ts`, the recording scenario.

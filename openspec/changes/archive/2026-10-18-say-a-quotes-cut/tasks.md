## 1. Server

- [x] 1.1 `quote_cut` on `MessageView`: the text cut, the HTML cut at 256 KiB, or the words at their limit
- [x] 1.2 Test: long HTML read in time is `quote_cut`, not `truncated` or `text_unread`; short HTML is neither

## 2. Client

- [x] 2.1 The composer's "first part" note on `quote_cut`
- [x] 2.2 The reply dialog's preview says when the quote is cut, or shown in part
- [x] 2.3 Tests; i18n in en, pt-PT, es-ES, fr; a `ReplyQuote` story with a note

## 3. Recording

- [x] 3.1 A long HTML demo email and the `quote-long-html` scenario

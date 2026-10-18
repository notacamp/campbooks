## 1. Reading

- [x] 1.1 End the head as the tree builder does (start tags, words)
- [x] 1.2 Fixtures: head and no body, doctype, closed head, Outlook; never script, style or head

## 2. Reindex

- [x] 2.1 HTML reading version in the stamp of messages read from HTML
- [x] 2.2 INDEX_VERSION 2: a background backfill on each workspace's next touch

## 3. Ingest

- [x] 3.1 Connector: a text part only from a real `text/plain` part
- [x] 3.2 One reading of a message's words (`blobs::words`) for every reader; blanks-only text falls back to the HTML
- [x] 3.3 Tests: HTML alone through the connector's sync, a blanks-only text part through ingest and search

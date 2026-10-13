# Recorded change batches

Fictional mail for tests and local development, in the four languages
Campbooks ships in. Every person, company and address is invented; domains
are `example.com` or end in `.example`.

Each file is a *recorded* `MessageChangeBatch` v1 (see
`contracts/message-change-batch.v1.schema.json`): bodies (`text`, `html`,
`raw`) are inline strings and each attachment has a `content` string instead
of a `blob`. `campbooks_conversations::dev::record` stores them as blobs and
returns the batch a connector would produce; the contract test validates
that against the schema.

| File | What |
| --- | --- |
| `en.json` | A colleague's request with a deadline and its reply (provider thread key), a newsletter, a security alert |
| `pt-PT.json` | A water bill (fatura, PDF), a message from a friend, a promotion |
| `es-ES.json` | A supplier invoice (factura, PDF), a mailing-list digest |
| `fr.json` | A shipping update, a receipt (reçu), a neighbour's message |
| `followup.json` | Later changes to `en.json`: read, starred, archived, relabelled, one message removed |
| `needs.json` | What needs Sam: a question with a date, a lease to sign, a bill, a phishing message, a lunch question |
| `needs-replied.json` | Sam's reply to the lunch question, sent from another client |
| `late-parent.json` | Two replies stored before the message they answer (no thread keys) |

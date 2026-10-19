# ai-decide-facts Specification

## Purpose
What deterministic code may know about an email (ADR 0018): its structured
facts (addresses, the list and automation headers, the receiving provider's
authentication results, attachment media types), never its text. From them
the facts strategy may only answer "no", and tells a `decide` model the same
facts as hints; understanding what an email says is the model's job.

## Requirements

### Requirement: Facts only say no
The facts strategy (`EmailFacts`) SHALL read only structured facts of a
message: the sender's and recipients' addresses, the headers `List-Id`,
`List-Unsubscribe`, `Precedence`, `Auto-Submitted`, `Authentication-Results`
and `Reply-To`, and attachment media types. It SHALL answer only
`needs_reply`, only false, with confidence 0.99 and one reason:
`facts.list_mail` (a `List-Id` or `List-Unsubscribe` header, or `Precedence`
`bulk`, `list` or `junk`), `facts.automatic` (`Auto-Submitted` other than
`no`) or `facts.no_reply_sender` (a no-reply local part). It SHALL abstain on
every other question and when no fact applies.

#### Scenario: A newsletter with a question
- **WHEN** a message with a `List-Unsubscribe` header says "What did you think? Hit reply!"
- **THEN** the facts answer `needs_reply` false with reason `facts.list_mail` and abstain on `asks_action`

#### Scenario: A person's question
- **WHEN** Rui writes from rui@costaobras.example "Do you want me to order the tiles?" with no list or automation headers
- **THEN** the facts abstain on every question

#### Scenario: A no-reply sender
- **WHEN** a message comes from no-reply@clouddrive.example
- **THEN** the facts answer `needs_reply` false with reason `facts.no_reply_sender`

### Requirement: No content rules
No deterministic code in `campbooks-ai` or a module SHALL decide what a
message means from its subject, body or attachment names: no phrase lists,
keyword lists or regular expressions over content, in any language. The
facts strategy's input SHALL hold no subject, body or attachment name, and a
test SHALL fail if its source reads them. Turning HTML into text and dropping
quoted history are not decisions and remain allowed.

#### Scenario: The facts module reads a subject
- **WHEN** a change makes `signals/facts.rs` read an envelope's `subject` or `body`
- **THEN** the facts module's source test fails

### Requirement: A copied recipient is a hint, never a decision
The facts SHALL note whether one of the inbox's addresses is in `Cc` and none
is in `To` (addresses compared without regard to case; no hint when the inbox
is unknown). The note SHALL reach the `decide` model as a hint, and the facts
strategy SHALL NOT answer any question because of it: whether a copied person
is asked something directly is for the model to read.

#### Scenario: Copied only
- **WHEN** the inbox's address is in `Cc` and not in `To`
- **THEN** the hint is set and `needs_reply` is still not answered by the facts

#### Scenario: Addressed
- **WHEN** the inbox's address is in `To`, also in `Cc` or not in either
- **THEN** the hint is not set

### Requirement: A configurable decide endpoint path
`CAMPBOOKS_AI__DECIDE__PATH` SHALL set the path the `decide` request is posted
to, relative to `BASE_URL`; when unset it SHALL be `systemone` for
`PROTOCOL=laya` and `chat/completions` for `PROTOCOL=openai`. Setting it on
any other role SHALL be a configuration error.

#### Scenario: Jev through OpenRouter
- **WHEN** `decide` is configured with `PROTOCOL=laya`, `BASE_URL=https://openrouter.ai/api/alpha`, `PATH=decisions`, `MODEL=typesafe/jev-1.13`
- **THEN** decide requests are posted to `https://openrouter.ai/api/alpha/decisions` with the Laya request body and the key as a bearer token

#### Scenario: Laya unchanged
- **WHEN** `PATH` is not set and `PROTOCOL=laya`
- **THEN** decide requests are posted to `<BASE_URL>/systemone`

### Requirement: Authentication results are read strictly
The facts SHALL read the receiving provider's `Authentication-Results` as RFC
8601 says. A value longer than 8 KiB (8192 bytes) SHALL be unread and SHALL
count as authentication failed (a sender may not pad a header to hide a
failure), and so SHALL one that is present but cannot be read (an unbalanced
quote or comment a sender echoed into it); only an absent one, or one whose
only result section is `none` (RFC 8601: the receiver checked nothing),
says nothing. The whitespace RFC 8601 allows around `=` and `/` and a comment
before `=` SHALL be read. The sender's domain SHALL count as authenticated only by exactly one
`dmarc` result, `pass`, whose `header.from` is the From domain; or by a
`dkim=pass` whose signing domains all have the From domain's registrable
domain, while no `dmarc` result in the header refuses it: only `pass`, `none`,
`bestguesspass`, `temperror` and `permerror` are not refusals, so `fail`,
`quarantine`, `reject`, `policy` and any non-standard result are. A From
domain that is itself a public suffix SHALL authenticate nothing. A
registrable domain SHALL be read from the Public Suffix List, ICANN and
private sections (compiled in, never fetched), so that two names under one
public suffix (`x.com.br` and `y.com.br`, `a.github.io` and `b.github.io`)
are never the same organisation; a name under a top-level domain the list
does not know SHALL be its own suffix, and a name the list cannot split SHALL
be kept whole. The RFC 2606 documentation domains (`example.com`,
`example.org`, `example.net`) SHALL stay suffixes so fixtures can stand for
many organisations. A taught `sender_domain` SHALL NOT name a public suffix
alone.

#### Scenario: A signature DMARC refused
- **WHEN** a message carries `dkim=pass header.d=studio.example` for a From of `studio.example` and a `dmarc` result of `fail`, `quarantine`, `reject`, `policy` or any other non-standard value, or two `dmarc` results of which one is `fail`
- **THEN** the hints say authentication failed

#### Scenario: A signature and no DMARC result
- **WHEN** a message carries `dkim=pass header.d=studio.example` for a From of `studio.example` and no `dmarc` result, a `dmarc` pass, or a `dmarc` of `none`, `bestguesspass`, `temperror` or `permerror`
- **THEN** the hints do not say authentication failed

#### Scenario: A padded results header
- **WHEN** the provider's `Authentication-Results` is longer than 8192 bytes
- **THEN** the hints say authentication failed, while one of exactly 8192 bytes is read as usual

#### Scenario: A signer under a shared suffix
- **WHEN** a message from `ana@y.com.br` is signed only by `x.com.br`, or one from `ana@b.github.io` only by `a.github.io`
- **THEN** the hints say authentication failed, while mail from `mail.y.com.br` signed by `y.com.br` is authenticated

#### Scenario: An unreadable results header
- **WHEN** the provider's `Authentication-Results` is present with an unbalanced quote or comment, or a result without `=`
- **THEN** the hints say authentication failed, while an absent header says nothing

#### Scenario: A receiver that checked nothing
- **WHEN** the provider's `Authentication-Results` is `mx.example.com; none`
- **THEN** the hints say nothing, while `mx.example.com;` with no result section, or `none` beside another result, says authentication failed

#### Scenario: Real provider headers
- **WHEN** the headers recorded in the repo's fixtures and modelled on Gmail and Zoho are read
- **THEN** every one parses

### Requirement: Who may be awaited
The facts SHALL work out who on an email may be awaited, only by removing
people: from the email's `To` then `Cc`, in order, leave out the inbox's own
addresses, the sender, and every list, no-reply or service address
(`is_machine_sender`), drop duplicates without regard to case, and keep at
most six. A sender that is a list, no-reply or service address SHALL never
be awaited either. The facts SHALL answer neither `next_move` nor `awaited`:
whose turn it is is for the model to read.

#### Scenario: A colleague and a no-reply address
- **WHEN** João writes to Ana Costa, copying the inbox and `no-reply@tickets.example.com`
- **THEN** Ana Costa is the only person who may be awaited, and neither question is answered by the facts

#### Scenario: Nobody to pick
- **WHEN** Marta writes only to the inbox
- **THEN** nobody may be awaited and the `awaited` question is not asked

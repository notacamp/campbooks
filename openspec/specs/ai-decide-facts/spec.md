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

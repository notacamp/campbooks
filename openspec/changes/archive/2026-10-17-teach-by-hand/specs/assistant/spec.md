# assistant Specification

## MODIFIED Requirements

### Requirement: Compiling an instruction into taught rules
`POST …/assistant/compile` (`assistant.compile_instruction`,
`assistant:write`, not mutating) SHALL send the `complete` role, through
the provider that may receive content, only one JSON object with the
sanitised sentence, the member's own inbox addresses, the stream keys and
the member's label names (read as the member through
`conversations.list_inboxes` and `conversations.list_labels`), with a fixed
instruction, a strict JSON schema, no tools, temperature 0 and room for a
reasoning model to think before it answers (up to 4096 tokens). It SHALL
read an answer wrapped in one Markdown code fence (info string empty or
`json`) as the JSON inside, and repair nothing else. It SHALL keep a proposed
rule only when every condition field is one of `sender_address`,
`sender_domain`, `subject_contains`, `header_present`, `label`, `stream`;
every action one of `move_to_stream`, `archive`, `add_label`, `mark_read`,
`needs_reply`, `asks_action`, `surface`, at most one per slot; every
address or domain appears in the sentence or is the member's own; no
domain is a free-mail domain; every subject word appears in the sentence;
every label is the member's and every stream one of the four; with 1 to 4
conditions or a `who` phrase from the sentence, 1 to 4 actions and at most
3 rules. A `who` phrase SHALL come back with up to 5 candidates from
`conversations.list_senders` and SHALL never be resolved by the model.
The answer SHALL say `rules`, `unsupported` (with the reason `sends_mail`,
`deletes_mail`, `not_about_mail` or `too_vague`) or `unclear`, and SHALL
store nothing. It SHALL allow 20 calls per member per hour (429), answer
409 `compile_unavailable`, calling nothing, when the `complete` role is not
configured or may not receive content, and 503 `compile_failed` when the
model call fails or its answer is not the schema's JSON (empty, cut short or
prose), without the provider's or the model's words; such an answer SHALL
never be reported as `unclear`.

#### Scenario: Anything from my accountant
- **WHEN** a member writes "anything from my accountant always matters" and the model returns one rule with `who` = "my accountant" and action `surface: always`
- **THEN** the answer holds that proposal with candidates whose name, address or domain matches "accountant", ranked by how often the member replied, and nothing is saved

#### Scenario: Two firms named without an address
- **WHEN** a member writes "anything from my accountants at northtax or atelier conseils always matters" and the model returns two rules, `who` = "northtax" and `who` = "atelier conseils", each with `surface: always`
- **THEN** the answer holds two proposals, each asking who it is with the senders matching its words

#### Scenario: An address in the sentence
- **WHEN** a member writes "always show mail from rita@silva-contas.example"
- **THEN** the answer holds `sender_address` = `rita@silva-contas.example` → `surface: always`

#### Scenario: An invented address
- **WHEN** the model returns a condition on `boss@corp.example`, which is not in the sentence nor the member's
- **THEN** that rule is dropped, and with nothing left the outcome is `unclear`

#### Scenario: Forwarding
- **WHEN** the model answers `unsupported` with reason `sends_mail` for "forward invoices to my accountant"
- **THEN** the answer says `unsupported` / `sends_mail` and holds no rule that sends anything

#### Scenario: An answer in a code fence
- **WHEN** the model wraps its JSON in ` ```json ` … ` ``` `
- **THEN** the answer is read as if the fence were not there

#### Scenario: An answer that is not JSON
- **WHEN** the model's answer is empty, cut short because it ran out of tokens, or prose
- **THEN** the response is 503 with code `compile_failed`, never the outcome `unclear`, and holds none of the model's words

#### Scenario: No language model
- **WHEN** the `complete` role is not configured
- **THEN** the response is 409 with code `compile_unavailable` and no model is called

#### Scenario: An external provider without the opt-in
- **WHEN** the `complete` role points off the instance's network without `CAMPBOOKS_AI__COMPLETE__EXTERNAL_CONTENT=allow`
- **THEN** the response is 409 with code `compile_unavailable` and nothing is sent

### Requirement: Nothing personal in telemetry or logs
Spans, logs and metrics of this module SHALL never hold the member's text,
an address, a label name, a model response or a provider's error body;
compile SHALL record only its outcome, latency and counts, why the model
stopped (one of `stop`, `length`, `content_filter`, `tool_calls`, `other`,
`none`), how many tokens it wrote and how many characters its answer had, and
for an answer that does not parse, only the kind (`empty`, `truncated`,
`not_json`).

#### Scenario: A failed call
- **WHEN** the provider answers 500 with a body echoing the request
- **THEN** the response is a problem without that body, and no log line holds it

#### Scenario: An answer cut short
- **WHEN** a reasoning model stops for length before its JSON is complete
- **THEN** the log says `truncated` with `finish_reason` `length` and the sizes, and holds nothing the model wrote

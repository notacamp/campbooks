## MODIFIED Requirements

### Requirement: No content rules
No deterministic code in `campbooks-ai` or a module SHALL decide what a
message means from its subject, body or attachment names: no phrase lists,
keyword lists or regular expressions over content, in any language. The
facts strategy's input SHALL hold no subject, body or attachment name, and a
test SHALL fail if its source reads them. Turning HTML into text and dropping
quoted history are not decisions and remain allowed. Reading data a sender
structured for machines (an iCalendar part, or schema.org markup in JSON-LD
or microdata) is not reading content (ADR 0020): its typed fields MAY
assert a dated item or ask, only with a proper parser of that format, only
for a sender whose domain passed DMARC aligned, and never from the prose
around it.

#### Scenario: The facts module reads a subject
- **WHEN** a change makes `signals/facts.rs` read an envelope's `subject` or `body`
- **THEN** the facts module's source test fails

#### Scenario: A booking's markup
- **WHEN** an authenticated hotel's email carries a `LodgingReservation` with check-in on 6 November
- **THEN** a `stay` item may be stored from its typed fields, and the email's words are not read for it

#### Scenario: A date in the prose
- **WHEN** a change looks for dates in an email's text with a pattern
- **THEN** it breaks this rule, whatever the language

## MODIFIED Requirements

### Requirement: Typed decision call
`campbooks-ai` SHALL offer one `decide` call that takes a context naming the
decision, a state (text or JSON) and a list of questions, each with an id
unique within the call and one of three types: `choice` (one label of an
ordered set of labelled options), `score` (one level of an ordered list of
level descriptions) and `noul` (whether a statement holds). It SHALL answer
every question of the call in one provider request and return one answer per
question, in question order, except that a question of the action catalogue
(an `action_*` id) whose answer is missing or does not fit its type SHALL be
left unanswered instead of failing the call. Any other question whose answer
is missing or does not fit SHALL fail the call, as before. An answer SHALL carry its typed value, the
probability of each option in question order when the provider can tell
(`[false, true]` for `noul`), a confidence equal to the probability of the
reported answer, the model that answered, the model version when the provider
reports one, and the latency of the call. Callers SHALL NOT receive free text
from `decide`.

#### Scenario: Three question types in one call
- **WHEN** a caller asks a `choice`, a `score` and a `noul` question about one state
- **THEN** exactly one request reaches the provider and three typed answers come back in the order asked

#### Scenario: No questions
- **WHEN** a caller asks no question
- **THEN** no request is sent and the answers are empty

#### Scenario: A catalogue answer that does not fit
- **WHEN** the provider leaves out `action_remind`, answers `action_forward` with 1.5 and `action_paid_automatically` with "NaN", and answers the triage questions well
- **THEN** the call succeeds, the triage answers come back as usual and the three catalogue questions have no answer

#### Scenario: A triage answer that does not fit
- **WHEN** the provider answers a triage `noul` question with "NaN" or leaves it out
- **THEN** the call fails, as before

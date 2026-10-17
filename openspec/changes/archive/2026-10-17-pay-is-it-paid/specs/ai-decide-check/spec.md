## MODIFIED Requirements

### Requirement: Answers check
When the `decide` role is configured and may receive mail, every server
process SHALL ask it the seven triage questions about ten fictional emails
with known answers (50 expected answers; each yes/no question has a yes case
and a no case, and each `money` option, `owed`, `paid` and `neither`, has a
case) at startup, then again 6 hours after a pass and 15 minutes after a
failure, until shutdown, and never per request. The check SHALL pass when at
least 90% of the expected answers are right, SHALL fail below that, and SHALL
be unavailable when the provider does not answer. Each check SHALL be logged
with its outcome and counts only, never the answers.

#### Scenario: A model that says yes to everything
- **WHEN** the decide model answers "yes" to every yes/no triage question
- **THEN** the check fails, since the newsletter, the receipt and the colleague's update each expect "no"

#### Scenario: The recorded Laya answers
- **WHEN** the check is graded on the laya-serve 0.3.21 multilingual answers recorded for the first eight emails, with the money question and the last two emails answered as a model that says yes to everything
- **THEN** it fails with 11 of 50 right

#### Scenario: A model that gets them right
- **WHEN** the model gets every expected answer right
- **THEN** the check passes with 50 of 50

#### Scenario: A bill and a confirmation
- **WHEN** the model answers `money` `neither` for every email
- **THEN** the bill and the two payment emails are wrong for `money`

## ADDED Requirements

### Requirement: Wait on them as a sentence
A rule that is exactly one sender, the member not in `To` and `wait_on_addressed`
SHALL read, in the rules list, in the smart-rule confirmation and in Scout's
memory, as "Mail from ana@studio.example to someone else with you in Cc: wait
on them." in the user's language. Any other rule using the condition or the
action SHALL read through the usual sentence ("Mail from x with you not in To →
Scout waits on the person it was written to"), and a conversation it handled
SHALL say "Campbooks is waiting on the person this was written to".

#### Scenario: In the rules list
- **WHEN** the list holds the rule for ana@studio.example
- **THEN** its sentence is "Mail from ana@studio.example to someone else with you in Cc: wait on them."

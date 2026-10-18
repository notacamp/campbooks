## ADDED Requirements

### Requirement: Keeping only the rule that was shown
Keep this rule SHALL send the rule's `content_token` from the list on
screen. On 409 `content_changed` the rules list SHALL say "This rule changed
since you opened it. Check it and keep it again." in all four locales, read
the rules again so the row shows the rule as it is now, still waiting for a
yes, and SHALL NOT keep it on its own.

#### Scenario: A proposal changed since it was listed
- **WHEN** Sam presses Keep this rule and the server answers 409 `content_changed`
- **THEN** the notice shows, the row reads what the rule does now, and it waits for Sam's yes

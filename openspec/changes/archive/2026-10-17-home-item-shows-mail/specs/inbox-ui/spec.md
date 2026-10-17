## ADDED Requirements

### Requirement: Links in mail that may be phishing
In a conversation whose need is a warning (`suspicious`), the reader SHALL
show each message body with links that lead nowhere: the frame SHALL NOT
allow popups (so no link can open a tab or navigate) and links SHALL not look
clickable. Every other protection of the body stays as it is.

#### Scenario: A phishing message in the reader
- **WHEN** the user opens a conversation Campbooks flagged as possible phishing and clicks a link in it
- **THEN** nothing opens

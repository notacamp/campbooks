## ADDED Requirements

### Requirement: Links in mail that may be phishing
In a conversation whose need is a warning (`suspicious`), the reader SHALL
show each message body with links that lead nowhere: once the body has
loaded, each link SHALL become plain text (its address kept aside, not as a
link, so it cannot be clicked, focused or opened from the browser's menu,
whatever the mail's styles say), and the frame SHALL NOT allow popups, also
while a frame shown before the warning arrived is replaced. Every other
protection of the body stays as it is.

#### Scenario: A phishing message in the reader
- **WHEN** the user opens a conversation Campbooks flagged as possible phishing and clicks a link in it
- **THEN** nothing opens

### Requirement: Plain-text mail keeps its line breaks
A message shown as plain text SHALL keep its own line breaks and paragraphs,
in the reader and wherever the message is shown (Home's opened item), and a
long word or link SHALL wrap instead of widening the page.

#### Scenario: A plain-text letter
- **WHEN** the user opens a plain-text email written as three short paragraphs
- **THEN** it shows as three paragraphs, not one run-on line

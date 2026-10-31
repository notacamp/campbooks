## ADDED Requirements

### Requirement: Ask Scout about a page
With mail and Scout in pages on, a page's More menu SHALL offer "Ask Scout",
which opens a Scout chat about that page. It SHALL be hidden when Scout is
unavailable, and the page toolbar SHALL stay one row.

#### Scenario: Asking about a page
- **WHEN** Ana chooses "Ask Scout" on "Client onboarding" and asks what is still open
- **THEN** Scout's chat is about that page and answers from it

#### Scenario: Scout not configured
- **WHEN** the `complete` role is not configured
- **THEN** the page's More menu has no "Ask Scout"

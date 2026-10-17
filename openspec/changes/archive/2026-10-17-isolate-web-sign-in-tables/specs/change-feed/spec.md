## MODIFIED Requirements

### Requirement: Backpressure and limits
The stream SHALL read changes in bounded pages of changes the caller may
see, reading the next page only once the previous one has been sent, and
SHALL hold no database connection while waiting. The server SHALL limit
open streams per caller and answer 429 beyond the limit, and SHALL limit how
many streams a caller opens per configured window (across processes),
answering 429 `rate_limited` beyond it, whatever happens in the workspace.
That window SHALL be between one second and a day, checked at startup.

#### Scenario: Many subscribers
- **WHEN** many clients subscribe to the same workspace and one change is recorded
- **THEN** every client receives it once

#### Scenario: Reconnecting in a loop
- **WHEN** a caller opens more streams than the configured number within the window
- **THEN** the next open answers 429 `rate_limited`, and other callers are not affected

#### Scenario: A window longer than a day
- **WHEN** `CAMPBOOKS_CHANGES__OPENS_WINDOW_SECONDS` is 0 or 86 401
- **THEN** the server refuses to start, naming the setting

## MODIFIED Requirements

### Requirement: Size limit
The system SHALL refuse bodies larger than `CAMPBOOKS_BLOBS__MAX_OBJECT_BYTES`
(default 52,428,800 bytes), whether given in memory or as a stream, and SHALL
store nothing for them. An operation refusing an over-limit upload SHALL answer
413 problem+json `file_too_large`, whose detail names the limit in bytes, in the
caller's language. A JSON request body over an operation's body limit SHALL be
answered 413 `body_too_large`, not 400.

#### Scenario: One byte over
- **WHEN** the limit is 1,000 bytes and a 1,001-byte stream is stored
- **THEN** the call fails as too large and nothing is stored

#### Scenario: The answer to an over-limit upload
- **WHEN** an operation's upload is refused as larger than a 1,048,576-byte limit
- **THEN** the response is 413 problem+json with `code` = `file_too_large` and a detail naming 1048576 bytes, in English, European Portuguese, Spanish or French as the caller asked

#### Scenario: An over-limit JSON body
- **WHEN** a JSON body is larger than its operation accepts
- **THEN** the response is 413 problem+json with `code` = `body_too_large`

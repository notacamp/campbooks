## ADDED Requirements

### Requirement: Reads that keep their inputs out of URLs
An operation that does not mutate but takes personal data as input (addresses,
subjects) SHALL be served by POST with its inputs in the body and declare
`x-campbooks.query_in = "body"` (`Operation::body_query()`), so the data never
appears in a URL, an access log, a proxy or browser history. `cargo xtask
arch` SHALL accept a non-mutating POST only with that marker, and SHALL
refuse the marker on a mutating operation or another method.

#### Scenario: Previewing a rule
- **WHEN** the catalogue lists `conversations.preview_rule`
- **THEN** it is a POST with `mutates: false` and `query_in: "body"`, and the arch check passes

#### Scenario: A read POST without the marker
- **WHEN** an operation is a POST with `mutates: false` and no `query_in`
- **THEN** `cargo xtask arch` reports that it must be GET or HEAD, or marked

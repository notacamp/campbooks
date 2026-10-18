## ADDED Requirements

### Requirement: A handler knows who invoked it
The `Workspace<S>` extractor SHALL tell a handler who ran the request
in-process (`invoked_by`: the assistant or automation, through `invoke`) or
that it arrived over HTTP (`None`). The kernel SHALL set it only from a
request extension that `invoke` attaches, private to the kernel: no
header, token claim or body can set or clear it, and no module can make
one. A request the idempotency layer authenticated SHALL keep how it
arrived.

#### Scenario: The assistant calls an operation
- **WHEN** the assistant invokes an operation for Sam in his workspace
- **THEN** its handler sees `invoked_by` as the assistant, with Sam's access

#### Scenario: Sam's own request
- **WHEN** Sam's client calls the same operation over HTTP with his token, whatever headers it sends
- **THEN** its handler sees `invoked_by` as `None`

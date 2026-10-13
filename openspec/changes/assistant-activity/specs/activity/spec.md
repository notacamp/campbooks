## ADDED Requirements

### Requirement: Sessions and steps
The `activity` module SHALL keep, per workspace, sessions of assistant work
and their steps in schema `activity`. A session SHALL have a kind named
`<module>.<kind>`, a trigger (`sync`, `member`, `rule`, `schedule`,
`assistant`), an audience (a container, a member, or the workspace), a status
(`running`, `done`, `failed`, `stalled`), its start, last update and end, the
roles and models its steps used, token counts, outcome counts and its
subjects (record references). A step SHALL have a subject, a stage, a state
(`working`, `done`, `skipped`, `failed`), and optionally a role, strategy,
model, latency, token counts, non-personal facts, references to the records
it produced, and a stable error code.

#### Scenario: A triage batch
- **WHEN** a sync batch of three inbound messages is applied with the `decide` role configured
- **THEN** one `conversations.triage` session exists within that inbox, with three subjects, three rules steps that are done, and the model stage pending for each message

### Requirement: Recording through the kernel port
Producers SHALL record sessions and steps only through the kernel's
`Activity` port, in a transaction they pass, so that a step commits or rolls
back with the producer's own change. A producer MAY record a step as
`working` in a short transaction of its own before a slow call and finish
the same step later; the step SHALL then be updated, not duplicated. When no
activity module is registered, the default port SHALL discard entries
without error.

#### Scenario: Rolled back with the decision
- **WHEN** the model pass stores its answers and its transaction rolls back
- **THEN** the model step is not recorded as done

#### Scenario: Working, then done
- **WHEN** the model pass records its step as working, calls the model, and stores the answers
- **THEN** the session's timeline shows one model step for that message, first working, then done

#### Scenario: Without the module
- **WHEN** `conversations` runs with the default port
- **THEN** its passes run and store their decisions exactly as before

### Requirement: Later stages join the session covering their subject
A step recorded for a subject with a session kind and a stage SHALL join the
newest session of that kind in which that stage is still pending for that
subject, and SHALL clear the pending stage. When no such session exists, the
step SHALL open a session of its own. A session SHALL be `running` while any
subject has a pending stage or any step is working, and SHALL then become
`done`.

#### Scenario: The model pass joins the batch
- **WHEN** the model answers about the second message of a three-message batch
- **THEN** its step is in the batch's session and that message has no pending stage left

#### Scenario: The last message
- **WHEN** the model step for the last pending message is recorded as done
- **THEN** the session is `done` with an end time and its outcome counts three subjects

#### Scenario: Skipped
- **WHEN** the model pass finds a pending message already answered or gone
- **THEN** it records the step as `skipped` and the session can finish

### Requirement: Failures and stalls
A producer whose work fails on its last attempt SHALL record the failure: a
failed step with a stable error code, or a failed session when the work as a
whole failed. A step left `working` longer than
`CAMPBOOKS_ACTIVITY__STALL_MINUTES` (default 15) SHALL be marked failed with
code `stalled`, and its session `stalled`. A pending stage that has not
started SHALL never be marked stalled.

#### Scenario: The model is down
- **WHEN** the decide provider answers 503 on every attempt for a message
- **THEN** on the last attempt the model step is `failed` with code `provider_unavailable` and no provider text is stored

#### Scenario: A worker died mid-call
- **WHEN** a model step has been working for longer than the stall limit
- **THEN** the sweep marks it failed with `stalled` and the session `stalled`

### Requirement: No personal data in activity
Sessions and steps SHALL hold ids, record references, kinds and stages from
the producer's code, option labels from the question's own set, numbers,
reason rule ids, model names and stable error codes only. They SHALL NOT hold
message subjects, addresses, bodies, matched evidence, prompts, provider
error messages, tokens or secrets.

#### Scenario: A phishing look-alike
- **WHEN** the rules flag a message as suspicious because of a look-alike sender domain
- **THEN** the step stores the answer, its confidence and the rule id `signals.lookalike_domain`, and the domain itself is only in `conversations`

### Requirement: Reading sessions
`GET /api/v1/w/{workspace_id}/activity/sessions` (`activity.list_sessions`,
scope `activity:read`) SHALL list the sessions the caller may see, newest
first, with an opaque cursor, a limit of 1 to 100 (else 400) and an optional
status filter (`running`, `attention` for failed, stalled or unsure
outcomes). `GET …/activity/sessions/{session_id}` (`activity.get_session`,
`activity:read`) SHALL return the session and its steps in timeline order,
only the steps after `after` when given, at most `limit` (1 to 200).

#### Scenario: Incremental steps
- **WHEN** a client holding the first five steps reads the session with `after` set to the fifth step's id
- **THEN** it receives only the steps recorded after it

#### Scenario: A malformed cursor
- **WHEN** the list cursor is not one the API gave
- **THEN** the response is 400 with code `invalid_cursor`

### Requirement: Who may see a session
A session within a container SHALL be visible only to callers the kernel's
`Visibility` port admits for that container (inbox members; a client token
bound to the workspace). A member-private session SHALL be visible only to
that member and never to client tokens. A workspace session SHALL be
visible to every member. Workspace admins SHALL get no wider view. A session
the caller may not see SHALL be omitted from lists and SHALL answer 404 when
read.

#### Scenario: Someone outside the inbox
- **WHEN** a workspace member who is not a member of the Work inbox reads a triage session of that inbox
- **THEN** the response is 404, and the session is not in their list

#### Scenario: An admin
- **WHEN** a workspace admin who is not in the inbox lists sessions
- **THEN** that inbox's sessions are not listed

### Requirement: Live updates through the change feed
Every recording SHALL publish `activity.session_changed` (v1: workspace,
session, status; ids only) in the producer's transaction, declaring an
upsert of `activity.session` within the session's container to the change
feed. No other streaming endpoint SHALL carry sessions or steps.

#### Scenario: A step lands
- **WHEN** a model step is recorded as done
- **THEN** inbox members following the change feed receive an upsert of `activity.session` with the session id and no content

#### Scenario: Someone outside the inbox follows the feed
- **WHEN** a member who is not in the inbox follows the feed while the session runs
- **THEN** they receive no frame for it

### Requirement: Retention and purge
Sessions and their steps SHALL be deleted
`CAMPBOOKS_ACTIVITY__RETENTION_DAYS` (default 30, 1 to 365) after they
started, by an hourly `activity.prune` schedule. When
`workspaces.workspace_deleting` is published, the module SHALL delete the
workspace's sessions when its grace period ends. When
`workspaces.member_left` is published, the module SHALL delete that member's
private sessions in that workspace.

#### Scenario: Old sessions
- **WHEN** a session is older than the retention and the prune runs
- **THEN** it and its steps are gone and reading it answers 404

#### Scenario: Workspace purge
- **WHEN** a workspace is deleted and its grace period ends
- **THEN** its sessions and steps are gone and other workspaces' are kept

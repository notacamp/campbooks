## ADDED Requirements

### Requirement: Tasks from an email's asks
The handler `tasks.add_read_asks` SHALL consume `conversations.message_read`
v1 and, for a personal inbox, make each ask an open task for the inbox's
owner: origin `ask`, the ask's verb, due date and amount, the title from the
read's texts, and the conversation as its source. It SHALL do nothing for a
shared inbox and SHALL never call a model.

#### Scenario: The signed lease
- **WHEN** Ana's personal inbox gets "please send the signed lease by Friday" and the read publishes a `do` ask due Friday
- **THEN** Ana has an open `do` task "Send the signed lease", origin `ask`, due Friday, whose source is that conversation

#### Scenario: The shared inbox
- **WHEN** a read of a message in the Shared inbox publishes a `pay` ask
- **THEN** no task is created

### Requirement: An ask is stored once
An ask SHALL keep its key on the task it made, so a repeated delivery or a
replay of the same read creates nothing, and a task whose ask key is known
SHALL never be reopened or changed by the read.

#### Scenario: Delivered twice
- **WHEN** the same `message_read` is delivered twice
- **THEN** Ana has one task for the ask

### Requirement: An ask that is an open task
An ask whose `same_as` names an open task of the inbox's owner from the same
conversation or the same inbox SHALL create nothing, and
SHALL only give that task the ask's due date when it has none. A `same_as`
that names anything else SHALL be ignored. A `pay` ask of one read of a
message whose other read already made a `pay` task for the owner SHALL be
that task.

#### Scenario: The reminder
- **WHEN** EDP's reminder is read with a `pay` ask due 30 October that is the same as Ana's undated EDP task
- **THEN** no task is added and Ana's EDP task is due 30 October

#### Scenario: Ana's own date wins
- **WHEN** the same reminder is read but Ana had set the task due 28 October
- **THEN** the task stays due 28 October

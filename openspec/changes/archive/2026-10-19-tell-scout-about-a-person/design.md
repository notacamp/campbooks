# Design

## 1. Data ownership

The table `conversations.person_notes` belongs to the conversations module
(schema `conversations`): `id`, `workspace_id`, `user_id` (the member, a user
of the workspaces module, referenced by id only), `address` (lower-cased,
3 to 320 characters, checked by the table), `note` (1 to 500 characters),
`created_at`, `updated_at`, unique on `(workspace_id, user_id, address)`.
That key also serves listing a member's notes and deleting them by member or
by workspace, so no other index is needed (the second migration slot is
unused).

No event is published. Clients refetch the list after their own writes, and
a note is nobody else's, so the change feed has nothing to tell anyone.
Notes never go into events, job payloads, errors or logs: the handlers log a
flag or nothing, `forget_member` logs a count, and errors carry codes.

## 2. Private per member

A note is its member's alone, not shared with teammates. A teammate's
opinion of a person is personal data about that person, and about the
teammate; security comes first, and the owner's ask is about how the member
writes, not about the workspace. Workspace owners and admins get nothing,
like a draft or Scout's profile.

In the access catalogue (ADR 0017) `conversations.person_note` is a
workspace resource owned by its member (`Owner::Member`, read, write and
delete), in no inbox, with no grants. Lists and upserts ask
`authz::require_type`; deleting asks `authz::require` on the note, whose
facts are its owner. A client token acting for nobody gets 403
`person_notes_need_member`; an app a member registered acts as that member,
as it does for drafts and Scout's profile.

## 3. Keyed by address, never in a URL

The person is their address, the same one the reply code finds (the sender
of an inbound message, the first To of an outbound one) and checks as plain
(`domain::person::plain_address`, shared with the reply code). Addresses are
personal data and URLs reach access logs, proxies and history, so the
upsert takes the address in its body and the delete takes the note's id;
no operation has an address in its path or query.

The note is kept as plain text: line breaks normalised, control characters
other than line breaks removed, and invisible direction and zero-width marks
stripped (`domain::person::kept_char`, the rule the model's replies already
follow), trimmed, 1 to 500 characters. At most 500 notes per member: a new
one beyond that is 409 `person_notes_full`; writes for one member are
serialised with a transaction advisory lock, so the limit holds. The list
shows the name the address last gave in non-spam mail to an inbox the caller
reads, when there is one (one query on `messages_inbound_sender_idx`).

## 4. Row-level security and per-member enforcement

The table enables and forces row-level security with the workspace policy
every conversations table uses
(`workspace_id = current_setting('campbooks.workspace_id')`), granted to the
runtime role.

The member is not enforced by the policy. `begin_for` sets only the
workspace; the only user setting the database knows, `campbooks.user_id`, is
set by the workspaces module's `begin_as_user` for its own cross-workspace
reads, never by a module's request transaction. Rather than invent a second
mechanism, the member is enforced in two other places: the access catalogue
(above), and every statement, which binds `user_id = <caller>` (list, upsert,
count, delete, and the reply's lookup). The jobs that delete notes
(`forget_member`, `purge_workspace`) run in the workspace's scope and delete
by member or by workspace. Tests prove a member of the same workspace can
neither list, change nor delete another's note (404), and the server's
access matrix checks the type against every kind of caller.

## 5. Drafting, and the distrust gate

`conversations.write_reply` (draft and rewrite; never
`conversations.suggest_replies`) looks up the caller's note for the person
answered, in the transaction that already reads the person, and sends it as
`my_note_about_them` in the user message's JSON, never in the system
message. The instruction says what it is, to follow it for tone, form of
address, language and formality above what the mail suggests, never to use
it as a fact unless the intent asks, and that it changes none of the rules
(no links, numbers, money and so on).

The note is read only when the person is known: a message answered that is
in spam, flagged suspicious, under review, or failing its sender's checks
gets no person context, and so no note, exactly like the earlier mail of
campbooks-cloud#271. A forged sender never gets the member's note about the
person it imitates. Role `complete`; when it is not configured or may not
read mail the reply is unavailable (409 `reply_unavailable`) as before, and
the dialog hides the line, since the note only steers Scout. The result is
a draft the member reads and edits; nothing is sent for them.

The dialog keeps the line for a conversation the client can't tell is
distrusted: the sender's checks are facts the client never receives, so a
partial gate there would promise what it can't keep. The server is the gate.

## 6. Scopes

- `conversations.list_person_notes`: `conversations:read`.
- `conversations.set_person_note`, `conversations.delete_person_note`:
  `conversations:write`, counted in the write rate-limit class like every
  member write. Mirrors the taught rules (`list_rules` reads,
  `create_rule` / `remove_rule` write), the member's other Scout memory.

## 7. Alternatives

a) **A workspace-shared note** that teammates see and edit. Useful for a
shared client, but it makes one member's opinion of a person everyone's
data, and needs roles and history. Rejected for now.

b) **Free-text "Teach Scout" sentences compiled into notes.** One place to
tell Scout everything; it needs the compiler to find the person and a
proposal to confirm. Later, maybe.

c) **A per-person formality picker** (formal, informal, language). Simple and
deterministic, but the owner wants the member's own words, which say far
more than a picker. Rejected.

d) **The note in the system message.** It would be followed more strictly,
and an instruction is the one place a member's words must never go. Rejected:
it travels as data, like the mail.

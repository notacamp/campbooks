## 1. Live refresh

- [x] 1.1 `conversations.get_message_body` reads `conversations.rule` (OpenAPI, `records.gen.ts`)
- [x] 1.2 The assistant's rule creation and removal refresh the bodies
- [x] 1.3 Test: a rule change or removal invalidates held bodies

## 2. Copy

- [x] 2.1 The address choice says what the checks vouch for, with the domain (en, pt-PT, es, fr)

## 3. Review round 1

- [x] 3.1 Without a live change feed (Tauri), bodies whose images depend on rules go stale after 60 s and refetch on focus; TODO names the native change source
- [x] 3.2 A rule's change re-reads only bodies with remote-image state; a message's change still re-reads every body of it
- [x] 3.3 The single address choice still shows what the checks vouch for

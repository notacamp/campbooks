# Tasks

## 1. The key

- [x] 1.1 `kernel::binding` (`Binding`, `Fields`, `TAG_VERSION`): HKDF-SHA256 from the instance secret with the label `campbooks/send-binding/v1`, HMAC-SHA256 tags with a version byte, constant-time verify, canonical length-prefixed encoding; a known-answer test
- [x] 1.2 `AppState::binding` (disabled by default), `with_binding`; the server derives it at startup and WARNs without a secret; the test state installs a fixed test key

## 2. Sends

- [x] 2.1 `send_draft` signs the author's request (`drafts.request_mac`); refuses without a key; `cancel_send` clears it
- [x] 2.2 `release_send` verifies it before it writes anything (WARN, no state change, no retry), stores `drafts.send_mac` over the stored send and clears the spent request binding
- [x] 2.3 The outbound-requests port verifies `send_mac` before a send stands

## 3. Changes

- [x] 3.1 `conversation::request` / `request_each` sign the recorded change request (`change_requests.mac`); the port verifies it before any item stands (`changes_standing` takes the state)
- [x] 3.2 `request_changes` signs each outbound change it inserts (`outbound_changes.mac`); a stream's run verifies each pending change and skips (`unverified`) and re-reads those that do not verify

## 4. Deploy

- [x] 4.1 Migrations `20261021230000` (drafts and change requests; scheduled drafts back to drafts) and `20261021230100` (outbound changes); rollback SQL in `design.md`

## 5. Tests

- [x] 5.1 Kernel: the format is pinned, tags verify only for their fields, context and key, the encoding is unambiguous
- [x] 5.2 Conversations: a planted draft (no, made-up or another draft's MAC) is never released and nothing is written; another generation, inbox or author; every edit of what goes out; the binding spent at release and on cancel; no key, no send
- [x] 5.3 Conversations, the port: a stored send with no or a made-up MAC, another document or author, or planted under another send id; a change request edited, planted or unbound
- [x] 5.4 Connectors: a planted or edited outbound change is never made at the fake Gmail; a member's change still is; existing tests that plant rows to set up a state bind them first
- [x] 5.5 Server, the real ports: a planted scheduled draft and release job are never released; a planted stored send is refused; a member's real send and changes still are made

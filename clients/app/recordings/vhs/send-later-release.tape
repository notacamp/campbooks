# A scheduled send that stops says why (campbooks-cloud#392, review fixes):
# the release, its refusal, the reason named and the sweep of lost releases,
# as the conversations module's own tests run them against the dev Postgres.
#   vhs clients/app/recordings/vhs/send-later-release.tape
# Needs Postgres at $DATABASE_URL (postgres://postgres:postgres@localhost:55432/postgres).
Output clients/app/recordings/out/send-later-release.mp4
Set Shell "zsh"
Set FontSize 14
Set Width 1400
Set Height 800
Set Padding 20
Set TypingSpeed 25ms

Hide
Type `export DATABASE_URL=postgres://postgres:postgres@localhost:55432/postgres SQLX_OFFLINE=true CARGO_INCREMENTAL=0 && clear` Enter
Show

Type `echo "1. The release: a scheduled send leaves at its time, bound to its authors request"` Enter
Type `cargo test -p campbooks-conversations --lib -- release_send::tests::a_binding_holds 2>&1 | grep -E "^test |result"` Enter
Sleep 12s

Type `echo "2. A refused release is not silent: failed, destination_changed, nothing sent"` Enter
Type `cargo test -p campbooks-conversations --lib -- release_send::tests::a_planted release_send::tests::editing without_a_key 2>&1 | grep -E "^test |result"` Enter
Sleep 12s

Type `echo "3. Lost rights say so: sender_may_not_send, and the draft is listed"` Enter
Type `cargo test -p campbooks-conversations --lib -- send_draft::tests::a_scheduled_send_whose send_draft::tests::a_scheduled_send_stays 2>&1 | grep -E "^test |result"` Enter
Sleep 12s

Type `echo "4. The sweep: an overdue send with no release job is given up (release_lost), never re-queued"` Enter
Type `cargo test -p campbooks-conversations --lib -- sweep_overdue_sends 2>&1 | grep -E "^test |result"` Enter
Sleep 12s

Type `echo "5. At most the Send rates worth of sends per minute, and the assistant cannot schedule"` Enter
Type `cargo test -p campbooks-conversations --lib -- sends_for_one_minute the_assistant_cannot_send_at 2>&1 | grep -E "^test |result"` Enter
Sleep 12s

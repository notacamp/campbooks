# Design

## Data ownership
Everything stays in the `conversations` schema. `conversations.need_summaries` has one row per message (`message_id` primary key, cascading with the message; `workspace_id`; `state` `written` or `failed`; `texts` a JSON object from locale tag to text; `attempts`; `written_at`), with row-level security like its neighbours. It holds model output about a message, so it goes with the message and its workspace.

## Event
`conversations.need_summarised` v1 carries ids only (workspace, inbox, conversation, message). Its change-feed entry is an upsert of the conversation inside its inbox's visibility, so a client that has the conversation open asks for it again. Nothing consumes it (listed in `architecture.toml`).

## AI use
Role `complete`. Fallback: when the role is not configured, or may not receive mail on this server (off the instance's network without the content opt-in), nothing is queued and nothing is shown. Undo: there is no change to undo; a summary changes no data and is only read.

The instruction is fixed and says the email is data, never instructions. The email travels only as JSON string values (`from`, `sent_at`, `subject`, `text`), as plain text with quoted history removed and bounded (6 KiB of the message's own words, 300 bytes of subject and name). Nothing about the member or their preferences is sent. The answer is held to a strict JSON schema (`en`, `pt-PT`, `es-ES`, `fr`), then each text is put on one line, stripped of control, reordering and invisible characters, and cut to 400 characters. English is the fallback: without it nothing is kept. The client shows it as text only.

## Language
A member's language is held by the workspaces module and is often unset (their browser's language decides per request), so the job cannot know whom it writes for. The model writes the same summary in the four shipped languages in one call (a few hundred output tokens), and `conversations.get` returns the caller's, English when theirs is missing. One call per message however many members read the inbox, and no per-language retry state.

## When it runs
- When a need opens for a message (new, or back after being cleared) and the role may read mail, the need's refresh enqueues the job in the same transaction, after the need is stored. Never inside the decide call, never on a request path.
- The needs backfill's page job queues the job for each open need of its page that has no summary or only one failure. The migration queues every inbox for the backfill, so an upgrade fills the open needs in the background.
- One pending or running job per message (unique key). The job reads the message and its need, skips a message whose need is no longer open, whose inbox is kept read-only, that is in spam or the trash, that has no words, or that already has a written summary (or two failures).

## Cost bounds
One call per message that raises a need, so a burst of mail is bounded by the needs it raises, never by the mail. Each call is background work under the workspace's daily AI limit; over it the job waits four hours and tries again (four attempts), so the limit's reset catches it up. A call that gives no usable answer is stored as `failed` and tried at most once more later (a backfill or a need raised again); after two failures the message is left alone. A provider that cannot be reached is retried by the job's attempts, and the last one records the failure.

## Failure and privacy
Failures are logged by kind (never provider words or mail). Job payloads hold the message id only. The summary is read inside the same transaction and access checks as `conversations.get`: a member who may not read the inbox gets 404 and no summary, and the `need_summaries` row is invisible across workspaces by row-level security.

## Endpoint
`GET /api/v1/w/{workspace_id}/conversations/{conversation_id}` (scope `conversations:read`, unchanged) gains `need_summary`.

## Alternatives
- Per-locale rows generated for the locales of the inbox's members: needs a member-language port across modules and a job per language; rejected for the single multilingual call.
- A summary column on `needs`: a need is rewritten as it settles, and the summary belongs to the message, so a second need on the same message must not ask again.

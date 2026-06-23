# Email Triage Ladder + Skim Mode

**Status:** Engine built and tested (41 specs green). **Skim Mode is LIVE at `/skim`**
— a self-contained page (new controller + view + route + nav entry) that deliberately
avoids the churning inbox. It pulls the real inbox, clusters it (`Emails::SkimBuilder`,
free) and renders the `SkimStack` story-viewer. Swipe (← archive / → keep / ↑ open) to
triage; flagged archives apply once via a confirmed "Archive flagged" button
(`Emails::SkimArchive`, security-scoped → `Tools::BulkArchive`); single-email clusters
get **Reply** straight into the email's full context. Mobile-responsive (no overflow at
375px). The remaining migration-gated work is folding `CategoryChip` + the `summary`
snippet into the *existing* `_thread_list`. See the hand-off checklist at the bottom.

## The problem

The inbox is ~98% machine noise. Measured over 10,243 real emails: Promos 1,381 ·
Notifications 217 · Finance 101 · … · Personal ~26. The mail a human actually
answers is under 2% of volume but is rendered in the same undifferentiated list
as everything else. Goal: triage the firehose cheaply, surface what needs a
person, and let the user skim/skip groups story-style — with AI as a proactive
protagonist, not a passive sidebar.

## The triage cost-ladder

Each email climbs only as far as it needs to; stop as soon as a rung is confident.

1. **Rules** — `Emails::Categorizer` (free, instant). Coarse category from
   sender/subject signals. Resolves ~21% as obvious noise.
2. **Embeddings** — `Emails::EmbeddingClassifier#shortlist` (~$0/email). Nearest
   workspace tag vectors (`SearchTagEmbedding`). Real-data calibration showed
   email↔tag cosine similarity tops out ~0.35, so embeddings produce a SHORTLIST,
   they do not auto-assign on an absolute threshold.
3. **Cheap model** — `Emails::LlmTagPicker`. A small/cheap model picks the best
   tag from the shortlist (a few names in, a number back).
4. **Full LLM** — `Ai::EmailClassifier` (the existing ~2-call path, incl. the
   security pre-screen). Only for important/sensitive mail or when rungs 1–3
   can't resolve it.

Orchestrated by `Emails::Triage`, which returns a pure `Decision`
(`category`, `confidence`, `tag`, `needs_llm?`). Wired into `EmailProcessJob`
behind a security-preserving, fail-safe gate: any rung that errors falls back to
today's exact behaviour, so ingestion never breaks.

**Clustering:** `Emails::Clusterer` groups a folder/group's vectors into stacks
(217 notifications → ~8 cards) for Skim Mode. Same cheap vectors, no LLM.

## Visual components (RubyUI/shadcn, electric-violet hue 276, light + dark)

- `Campbooks::CategoryChip` — triage category chip. `personal`=violet,
  `important`=amber, the four noise types=muted; icon **and** label (WCAG).
- `Campbooks::SkimRow` — skimmable inbox row: chip + sender + subject + one-line
  AI/snippet preview + unread dot.
- `Campbooks::SkimCard` — the cluster story-card: one decision over a whole stack
  (Archive all / Keep / Open; swipe + keyboard symmetric).

Previews: `/lookbook/preview/category_chip/...` and `/lookbook/preview/skim_card/...`.

**Design thesis:** visual weight encodes how much the email matters — noise
recedes (muted), personal/important surface (colour). Calm by default, show what
needs you (`PRODUCT.md`). One card = one decision over a cluster.

## Bugs fixed along the way

- `Emails::Categorizer` no longer treats `contact_id` as an importance signal
  (the app gives nearly every email a contact, so it had mislabelled 78% as important).
- `EmbeddingService` now falls back to the env `OPENAI_API_KEY` when the
  workspace adapter's stored key fails (it was returning 401 in dev, leaving
  embeddings dormant).
- `EmbedTagJob` called `.to_plain_text` on `Tag#prompt`, which the model already
  overrides to return a plain String — so every prompt-bearing tag silently
  failed to embed. Fixed; all 76 tags now vectorize.

## Hand-off checklist (post-migration)

1. **Commit** the engine + fixes together with the refactor — they're interleaved
   across `email_process_job.rb` / `embed_tag_job.rb` / `embedding_service.rb` /
   `schema.rb`, so there's no clean isolated commit (and `git add -p` is
   unavailable in the agent environment).
2. **Decide the rung-1 short-circuit policy:** coarse-category-only vs. mapping
   confident noise to a default tag (the ~21% cost lever). Rung 3 currently runs
   on ~99.5% of non-important mail because the 0.78 embedding auto-accept never
   fires given sims ≤0.35 — still a big cut from 2×Sonnet, but this is the lever
   to cut further.
3. **Wire the components into the migrated `_thread_list`:** `CategoryChip` + the
   free `summary` snippet in rows; fix `EmailMessagesController#index`, whose
   group-drilldown double-filters folder AND group and renders "No mail yet".
4. **Assemble Skim Mode:** a swipe/keyboard Stimulus controller over `SkimCard`,
   fed by `Clusterer` + cluster-summary copy; map the swipe actions onto the bulk
   tools that already exist (`Tools::BulkArchive`, `Tools::BulkTag`, etc.).

## Key files

- `app/services/emails/{categorizer,embedding_classifier,clusterer,llm_tag_picker,triage}.rb`
- `app/components/campbooks/{category_chip,skim_row,skim_card}.rb` (+ `*_variants` / `*_demo`)
- `spec/services/emails/*_spec.rb` (34 examples)
- `test/components/previews/{category_chip,skim_card}_component_preview.rb`
- `lib/tasks/email_triage.rake` — rules-only category backfill (read-only by default; `WRITE=1` to persist)
- `db/migrate/*_add_category_to_email_messages.rb` — `category` / `category_confidence` / `categorized_at`

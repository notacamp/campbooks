# Embeddings & semantic search

Semantic search (email search, document search, the Cmd+K palette, Scout's search
tools) and embedding-based tag classification run on **vector embeddings** stored
in Postgres via pgvector. Each workspace picks which embedding model produces
those vectors in **Settings → AI → Semantic search**.

## Supported models

The catalog lives in code at `app/models/ai/embedding_models.rb`:

| Model | Provider | Dimensions | Data region |
|---|---|---|---|
| `text-embedding-3-small` *(default)* | OpenAI | 1536 | US |
| `text-embedding-3-large` | OpenAI | 3072 | US |
| `gemini-embedding-001` | Google Gemini | 1536¹ | US |
| `mistral-embed` | Mistral | 1024 | **EU** |

¹ Requested at 1536 via the API's `outputDimensionality`; vectors are re-normalized
on our side.

`mistral-embed` matters for **EU data residency**: a workspace with the EU
residency policy enabled can't use OpenAI or Gemini, so it previously had no
embedding option at all — semantic search simply paused. Selecting Mistral keeps
processing in the EU.

The provider must actually be usable for the workspace: a configured API key in
Settings → AI (bring-your-own or Campbooks-managed), or — self-hosted only — the
operator's `OPENAI_API_KEY` / `GEMINI_API_KEY` / `MISTRAL_API_KEY` environment
key. There is deliberately **no cross-provider fallback**: vectors from different
models live in incompatible spaces, so falling back would silently corrupt
similarity scores.

## What happens when a workspace switches models

1. The choice is saved on the workspace and a background **re-embed sweep**
   (`Search::WorkspaceReembedJob`) starts: one low-priority job per workspace that
   processes the index in batches (chunks → search records → tag vectors) and
   re-enqueues itself until done. It never fans out per-item jobs, so a large
   re-embed can't crowd out user-facing work.
2. Every search row is stamped with the model that embedded it. Queries only
   match rows stamped with the workspace's current model, so **while the sweep
   runs, semantic search covers a growing subset of the corpus** — results are
   never wrong, just temporarily incomplete. Keyword/filter search is unaffected.
   Settings → AI shows the progress.
3. Switching again mid-sweep is safe: the sweep always converges on the latest
   selection. Switching back later re-embeds again (old vectors for a different
   dimension bucket are cleared as rows are rewritten).

Re-embedding calls the workspace's embedding provider once per batch of content,
so a switch costs real API usage proportional to the size of the mailbox/document
corpus. That's why it only ever happens on an explicit selection in Settings.

## Storage layout

Vectors are stored in **dimension-bucketed columns**: the original
`vector(1536)` columns plus nullable `*_1024` and `*_3072` siblings on
`search_chunks`, `search_records`, and `search_tag_embeddings`, with an
`embedding_model` stamp per row. Rows written before stamping existed (NULL
stamp) are treated as belonging to the default model.

- 1024/1536-dim columns have HNSW cosine indexes.
- 3072-dim columns are **not ANN-indexed** (pgvector's HNSW caps out at 2000
  dimensions for the `vector` type); `text-embedding-3-large` searches use exact
  scans, which are fine at per-workspace corpus sizes. A `halfvec` expression
  index (pgvector ≥ 0.7) is the follow-up if that ever gets slow.

Adding a model with a new dimension size means one catalog entry plus a
migration adding that dimension's columns; models that reuse an existing bucket
(another 1536-dim model, say) are catalog-only.

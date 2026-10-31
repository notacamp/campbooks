## Why

The owner asked for "a feature for AI to explain an email thread to me" (campbooks-cloud#407). Long threads with several people are hard to pick up again. Today nothing in Next explains a whole thread:
- Scout's read summarises one email in one or two sentences, and only when the email needs something.
- The chat about a thread sees only its newest four messages and keeps nothing between visits.
- "Where things stand" in the reader is a templated sentence.

## What Changes

- **Explain this thread** in the reader's "Where things stand" note, with `E`, and in ⌘K. It is shown only when Scout can read mail on this server and the thread raised no phishing warning.
- **One `complete` call, on request only, never in the background.** It reads the thread and answers, in the member's language:
  - **In short**;
  - **Who wants what**;
  - **Agreed**;
  - **Still open**;
  - **On you**.

  Each point names the messages it comes from. Long threads keep their newest messages in full and use Scout's per-message reads, or just the headers, for the rest.
- **Kept for the member until the thread changes.** After new mail it stays, marked as possibly out of date, with "Explain again". It is never redone on its own. It is never served once the member can't read every message it was made from. It is removed with the thread, the inbox, the member, or after 30 days.
- **Cost:**
  - charged as interactive work against the workspace's daily AI limit;
  - limited per member per hour;
  - bounded input;
  - a double click or a second tab never makes a second call.
- **"Ask a follow-up"** opens the Scout chat about the thread, which now also sees the member's explanation.
- **Scout's read of a message** stays the only *background* call that reads mail content. The explanation is the one call a member asks for.

## Capabilities

- `conversations`:
  - new: "Explaining a thread on request", "Who may see an explanation", "When an explanation is kept";
  - modified: "Scout's read of a message".
- `inbox-ui`: new "Explain this thread in the reader".
- `assistant-chat`: new "A chat about a thread knows the member's explanation".

## Impact

- **Server:** the `conversations` module gets the operations `conversations.explain`, `conversations.get_explanation` and `conversations.forget_explanation`, a job, and the table `conversations.explanations` (migration `20261122010000`). `assistant` gets one extra read in its about-context for conversations.
- **Tracking:** patch 0028 (plan version 28), with counts and durations only.
- **Client:** the reader, ⌘K, the shortcut catalogue and the Scout panel, in en, pt-PT, es-ES and fr.
- **Prototype:** https://claude.ai/artifact/69QS4V7Cqq6BFA57mAk3q3

## Non-goals

- Explaining automatically, in lists, on Home, or for every thread.
- Translating the thread itself, or drafting the reply. Reply drafting already exists.
- Sharing an explanation with teammates. It is the member's own.

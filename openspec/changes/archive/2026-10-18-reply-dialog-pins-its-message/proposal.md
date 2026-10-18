## Why

The pre-merge review of #524 found four gaps in the reply dialog
(campbooks-cloud#220):

- **Wrong message:** the dialog always answered the newest message, so mail
  arriving while the member reviewed a draft made Send reply to the new
  message with a draft written for the old one.
- **Dead Undo:** a send with no `send_at` still offered Undo, which could
  only end in "too late".
- **Reply without a composer:** Reply could do nothing when there was no
  composer.
- **Swallowed keys:** the dialog's document-level capture listener took
  every key whenever the focus fell to the page, blocking page shortcuts
  while any dialog was open.

## What Changes

- **Pinning:** the dialog pins the message it answers as soon as a draft is
  asked for, or when an undo hands one back. While nothing is written, it
  still follows the newest message.
  - If newer mail arrives, the dialog says "A newer message from <name>
    arrived. Your reply still answers the one quoted below." and offers
    "Answer the newer one".
  - "Answer the newer one" writes the draft again for the newer message
    (or re-quotes it when there is no intent). A draft already saved for
    the older message is never reused: it is replaced and discarded.
  - Send, the quote and Open in composer all follow the pinned message. The
    reader and Home open the composer on the draft's own message (Home
    passes it as `?to=`).
- **Undo:** with no send time, the toast says "Sending to <name>" without
  Undo.
- **Reply without a composer:** the reader already hid Reply, Reply all and
  Forward and disabled their keys without a composer. A test now holds that.
- **Dialog keys:** the dialog no longer listens on the whole document. When
  its focused control goes away, or the focus leaves for nowhere, it takes
  the focus back (a mutation observer and `focusout` on the dialog), so Esc,
  the numbers and the arrows keep working and plain keys still stop at it.
  A key the page itself receives is the page's.
- The demo Gmail can deliver Leo's answer in the Q4 thread
  (`POST /fake/deliver?reply=q4`), for the `reply-newer-mail` scenario.

## Impact

- Specs: home-ui (A reply answers the message its draft was written for;
  Dialog keys stay in the dialog).
- Code: `ReplyDialog`, `ConversationReader`, `InboxScreen`, conversation
  routes (`?to=`), `WhatNeedsYou`, `lib/ui` Dialog; i18n en, pt-PT, es-ES,
  fr; fake Gmail; the scenario.

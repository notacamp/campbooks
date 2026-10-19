## Why

The security review of trusted-sender images (#549) left follow-ups,
campbooks-cloud#236.

- **A removed rule kept showing images elsewhere.** The client caches
  message bodies indefinitely, so other devices and members, and the
  device that removed a rule from the assistant, kept showing that
  sender's images until a reload.
- **"This address alone." overstated what is checked.** The checks behind
  trust (DMARC, or a DKIM signature aligned with the From domain)
  authenticate the domain, not the part before the @. Anyone allowed to
  send DKIM-signed mail for that domain gets the images too.

## What Changes

- **Live refresh.** `conversations.get_message_body` declares that it reads
  `conversations.rule`. On the web, every live change of a rule re-reads the
  bodies a client holds whose images depend on rules (trusted, held back or
  offered), on every device and for every member who sees the rule. The
  assistant's rule creation and removal refresh the bodies on this device
  too.
  - We chose the live event over a short `staleTime`. It's immediate, it
    costs nothing while rules don't change, and a timer would re-fetch every
    open body for nothing while still leaving a window.
- **Desktop and mobile follow within a minute.** The desktop and mobile
  apps (Tauri) don't follow the change feed yet: their native change source
  isn't built (a TODO names it at `followsChangesLive`). Until it is, their
  bodies whose images depend on rules go stale after 60 seconds and are
  re-read when the window regains focus. Bodies without remote-image state
  never expire.
- **Honest copy.** The address choice now reads "Mail from this address
  when acme.example's checks pass. Those checks vouch for the domain, not
  for this one person." (en, pt-PT, es, fr). The address rule itself is
  unchanged.
  - We don't require SPF `smtp.mailfrom` to match. SPF authenticates only
    the envelope's domain, never its local part, and legitimate mail rarely
    carries the From address as its envelope sender (bounce addresses,
    VERP, sending services, forwarding with SRS). Requiring it would hold
    back most genuine mail and still prove nothing about the person.

## Impact

- **Spec:** inbox-ui (Always show images from a sender).
- **Code:**
  - `conversations.get_message_body`'s records (OpenAPI and
    `records.gen.ts`);
  - the assistant's `useCreateRules` and `useRemoveRules`;
  - the reader's address choice;
  - four locales.
- **No server behaviour change.**

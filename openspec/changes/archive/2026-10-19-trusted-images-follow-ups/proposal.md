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
- **A kept body re-showed trusted images (review round 2).** Marking the
  bodies stale was not enough: a body not on screen when the rule changed
  (a folded message, a conversation left, a desktop body not yet stale)
  rendered its kept HTML with its remote images on reopening, before the
  re-read answered.

## What Changes

- **Live refresh.** `conversations.get_message_body` declares that it reads
  `conversations.rule`. On the web, every change of a rule, made here or
  arriving live on every device and for every member who sees the rule, goes
  through one helper, `refreshBodiesAfterRules` (the change feed's reset drops
  the same bodies before re-reading everything):
  - it **drops** the kept bodies a rule trusted (`resetQueries`, not
    `invalidateQueries`), and any first read still on its way. Marked stale
    only, a folded message, a conversation opened again or a placeholder
    would render the kept HTML with its remote images before the re-read
    answered, and those images would tell a sender who is no longer trusted
    that the mail was opened. An open body shows its skeleton while it is
    read again;
  - it re-reads in the background the other kept bodies whose images are
    held back or offered (they fail safe).
  - We chose the live event over a short `staleTime`. It's immediate, it
    costs nothing while rules don't change, and a timer would re-fetch every
    open body for nothing while still leaving a window.
- **Desktop and mobile hold images until they check.** The desktop and
  mobile apps (Tauri) don't follow the change feed yet: their native change
  source isn't built (a TODO names it at `followsChangesLive`). A rule changed
  in the app drops the trusted bodies as on the web. For a change made
  elsewhere, a body a rule trusted is stale at once (`staleTime` 0), read
  again every minute while open and on focus, and the reader shows the kept
  body with its remote images held back (sources taken out before the frame
  parses it, and the frame's policy blocking them) until the read made as the
  message opened answers. Offline, they stay held back. Holding the images
  back keeps the message readable, which a skeleton would not. Bodies whose
  images are held back or offered go stale after 60 seconds and are re-read
  on focus; bodies without remote-image state never expire.
- **Exact copy.** The address choice now reads "Mail from this address when
  its domain's checks pass. Those checks vouch for the domain, not for this
  one person." (en, pt-PT, es, fr). It says "its domain" rather than naming
  the host: DKIM and DMARC align on the registrable domain (relaxed
  alignment), so the full host after the @ would be too narrow. The address
  rule itself is unchanged.
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
  - the change feed's cache binding, the assistant's `useCreateRules` and
    `useRemoveRules`, and the rule mutations, through
    `refreshBodiesAfterRules`;
  - the body query's freshness and the reader's image guard
    (`body-freshness.ts`, `ThreadMessage`, `SafeHtml`'s
    `holdRemoteImages`);
  - the reader's address choice;
  - four locales.
- **No server behaviour change.**

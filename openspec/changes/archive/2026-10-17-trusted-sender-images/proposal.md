# Proposal

## Why

Remote images stay held back until the member chooses Show images (the
2026-10-16 decision, campbooks-cloud#160). For senders they read every week
that is a click every time. The decision's follow-up: let a member trust a
sender so their images show on their own, as a smart rule, without opening
the door to a sender who only pretends to be that one.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/200

## What Changes

- Modules touched: `campbooks-ai` (taught rules engine, facts), `conversations`
  (rules, message body) and the client (reader, smart-rule confirmation,
  Scout's memory).
- A new rule action, `show_images`: one sender (an address, or a domain with
  its subdomains, never a free mail domain alone), and nothing else in the
  rule. It changes no mail: it is read when a message's body is opened, so
  there is nothing to apply to the mail already here.
- A message's remote images show without asking only when an active rule of
  its inbox trusts its sender **and** the receiving provider's results
  authenticate that sender (a DMARC pass for the From domain, or an aligned
  DKIM pass that no DMARC result refuses). A spoofed From, a failed DMARC
  check, results that can't be read and no results at all never pass; nor
  does mail that looks suspicious. Facts only (ADR 0018).
- Trust only flips what is held back by default: the same sanitiser runs,
  tracking pixels are removed either way, there is no proxy, and the
  desktop CSP is unchanged (campbooks-cloud#199 is separate).
- A rule that shows images needs mail from that sender in the inbox, so a
  member trusts only a sender whose mail they can see where they may teach.
- The body tells the client `images_trusted`, so the reader says why the
  images show.
- In the reader, beside Show images: **Always show images from <sender>**
  (⇧A). It opens the smart-rule confirmation (the sentence, the Smart rule
  tag, Scout's memory), where the member picks the address or anyone at its
  domain, then saves with Undo. Removing the rule in Scout's memory holds the
  images back again.
- Demo mailbox: the Weekly Ledger's newsletter passes DMARC, and a message
  pretending to be the Ledger fails it. Recording scenario `trusted-images`.

## Non-goals

- No image proxy and no fetching at sync (parked, campbooks-cloud#201).
- No change to the desktop CSP (campbooks-cloud#199).
- No AI anywhere in it.

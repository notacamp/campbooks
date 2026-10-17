# An opened Home item shows the email

## Why
The owner, on 2026-10-17: "in order to make a decision i need to see the email content when i click to expand it". Since ADR 0018 the decide model no longer quotes the sender, so an opened item showed only how it was decided and nothing of the email. The approved prototype (v6) shows the email itself, a few lines deep, with "Show the whole email".

## What changes
- Opening a Home item (Enter, or a click on the selected row) shows the message the need is about, rendered exactly as the reader renders it: server-sanitised, in the sandboxed frame, remote images held back behind "Show images", attachments listed. It shows a few lines deep with a fade, and "Show the whole email" opens it in place ("Show less" folds it back). The quote is no longer repeated when the email shows.
- In mail that may be phishing (a `suspicious` need), the body's links lead nowhere: the frame drops `allow-popups`, so a link cannot open anything, and links stop looking clickable. A line says so under the email on Home. The reader does the same for that conversation.
- A new `Clamp` component in `lib/ui` holds long content a few lines deep, with tokens for its height and fade.
- Scout's summary of the email is a separate change (campbooks-cloud#204).

## Impact
- Client only: `lib/ui` (`Clamp`, `SafeHtml` `inertLinks`, `ItemDetail` `mail` slot), `conversations` (`NeedMessage`, the reader passes inert links), `attention` (Home's opened item). No API or server change.

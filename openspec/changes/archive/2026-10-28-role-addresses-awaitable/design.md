# Design

## Why this only removes less (ADR 0018)

The facts may only say "no". Before, `awaitable` said no to three kinds of
party: cannot answer, is the inbox or the sender, is a role address. It now
says no to the first two only. Fewer vetoes means more options for the
`awaited` question, never a decision: the model still reads who has the move,
and the member still picks. Nothing reads text. The address and the display
name are header data.

A machine *sender* (list or automatic mail, a no-reply or service address)
still means nobody is awaited. `is_machine_sender` is unchanged.

## Cost

`awaited` is asked when `awaitable` is non-empty. Mail to a role address now
asks it where it did not. It is one more question with up to six options on
those messages, at the price measured in
`crates/ai/evals/decide/mail-triage/reports/2026-10-25-confidence-bands-and-moves-cost.md`.

## Display name

`facts::display_name(header_name, address)`, next to `awaitable`:

1. The header name, when present and not a role word.
2. Else the organisation: the first label of the registrable domain
   (`facts::registrable_domain`), split on `-` and `_`, each word capitalised
   (`aguas-ribatejo.example` gives "Aguas Ribatejo"). No accents are added.
   A free mail provider (gmail.com and the like) names no organisation.
3. Else the address.

A role word is a name that, lowercased with accents folded and trimmed, equals
the address's local part, matches the service pattern, or is one of a short
list of generic mailbox names in en, pt, es and fr (geral, general, info,
contact, contacto, contato, support, suporte, soporte, reservas, recepcion,
accueil, secretaria, office, admin, mail).

It runs where a party is serialised: `Unsure.others`, `Unsure.party` and
`NeedView.waiting_on`. Stored names (`awaited_name`) are not rewritten, so a
wait stored before this change is named correctly too. The client shows what
it gets.

## Data, events, AI

No schema change, no event, no endpoint, no scope. No AI use is added or
changed. The role is still `decide`; without it nothing is asked, as before.
Undo is unchanged: the member picks, and Undo withdraws.

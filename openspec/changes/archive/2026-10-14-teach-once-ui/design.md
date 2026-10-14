# Design

The whole teach-once design, its alternatives and the owner's decisions are
in `openspec/changes/archive/2026-10-14-taught-rules/design.md` (PR A). This
change is its client half; only the choices made while building it are here.

## The offer is a non-modal card, not a dialog or a toast

| | **a. A docked, non-modal card (`Offer`)** | b. A modal dialog | c. A bigger toast |
| --- | --- | --- | --- |
| The list behind it | stays usable: J, K, E keep working | blocked | usable |
| Room for choices and a preview | yes | yes | no |
| On phones | across the bottom, above the toasts | full screen | a line |

**Chosen: a.** It is a new `lib/ui` component (`Offer`, with `Progress` for
the run), both with stories in every state and tests. It reuses
`ChoiceCards` for the likeness and `Line` for the samples.

## Keys: 1 to accept, Esc to dismiss (not Y)

The prototype showed Y. Y is the members list's "copy invitation link" in the
same `item` scope, and the registry rejects the clash. The app already
answers a suggestion with 1 and 2 (the note's "Move to Newsletters" is 1), so
the offer's accept is `item.choice1` and its dismissal `overlay.close`
(Esc). The offer binds them after the reader, so they win while it shows.

## Where the state lives

A `TeachOnceProvider` wraps the inbox page; the inbox's actions
(`useInboxActions`) and the need actions (`useNeedActions`) call it after the
server confirms an action, and hand it any `rule_exception` an operation
returns. Outside the inbox (Home, search results) there is no provider and
nothing is offered.

## Progress comes from the change feed

The run's progress is `conversations.get_rule` (it also reads a removed rule,
so Undo all's revert can be followed); the change feed's `conversations.rule`
upserts refetch it. No polling.

## Copy

The app's own vocabulary ("Campbooks"), every name in the i18n catalogues,
four locales. A rule's work is said neutrally ("by a rule") wherever a
shared inbox could misattribute it.

## Does this copy the Rails app?

No: Rails had no offer after an action; its "Scout's memory" sentences are
the only kinship (the rules list), as the PR A design says.

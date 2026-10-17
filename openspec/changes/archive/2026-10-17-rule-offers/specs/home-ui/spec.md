## ADDED Requirements

### Requirement: Report and delete
A warning on Home SHALL offer "Report and delete" (1) before "It's safe":
`conversations.report_spam`, which moves the conversation to spam at the
provider, with the row's one-line confirmation and a toast whose Undo puts it
back where it was (`conversations.not_spam`). Once the server agreed, Home
SHALL ask `conversations.suggest_rules` with `action=report` and, when there
is an offer, show it docked above the toasts: "Never see emails like this
again?", what it would do in words (mail that says it is from the domain but
fails its sender checks, or the sender), "Yes, from now on" (1) and "Just
this one" (Esc, which dismisses the offer). Yes SHALL open the smart-rule
confirmation and never save on its own.

#### Scenario: A fake PayPal
- **WHEN** the member reports a phishing message from service@paypal.com that failed its checks
- **THEN** it leaves Home with Undo, and the offer asks "Never see emails like this again?" for "mail that says it's from paypal.com but fails its sender checks"

### Requirement: The paid rule offer
After Already paid on a bill, once the server agreed, Home SHALL ask
`conversations.suggest_rules` with `action=paid` and, when there is an
offer, show "Are <payee>'s invoices always paid for you?" with "Yes, from now
on" (1) and "Just this one" (Esc); Yes SHALL open the smart-rule confirmation
for "mark <payee>'s invoices paid". "All N already paid" offers nothing.

#### Scenario: EDP
- **WHEN** the member says EDP's invoice is already paid
- **THEN** the offer asks whether EDP's invoices are always paid, and Yes opens the confirmation of the paid rule

### Requirement: Scout noticed
At the bottom of Home, below the groups (and in place of nothing when
nothing needs the member), Home SHALL show at most one suggestion from
`conversations.list_rule_notices`: "Scout noticed", "n of N", what Scout
noticed in a sentence built from the counts in the member's language ("You
got 34 emails from builds@ci.example.com in the last 60 days and opened 2.
Keep them off Home and file them under Updates?"), and three buttons in one
style: "Set up a smart rule…" (opens the smart-rule confirmation with the
suggestion's rule), "Not now" and "Don't suggest this" (each with a toast and
Undo, and the next suggestion shows). In a shared inbox it SHALL say the rule
would act for everyone in that inbox. It SHALL not show while the list
loads or fails.

#### Scenario: Two suggestions
- **WHEN** Scout noticed build notifications and job alerts
- **THEN** Home shows "1 of 2" with the build notifications; Not now shows "1 of 1" with the job alerts, and Undo brings the first back

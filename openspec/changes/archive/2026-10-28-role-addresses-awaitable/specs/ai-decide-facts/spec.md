## MODIFIED Requirements

### Requirement: Who may be awaited
The facts SHALL work out who on an email may be awaited, only by removing
people: from the email's `To` then `Cc`, in order, leave out the inbox's own
addresses, the sender, and every address that cannot answer (a no-reply,
bounce, mailer-daemon or postmaster address), drop duplicates without regard
to case, and keep at most six. A role address (`info@`, `support@`, `geral@`)
SHALL stay awaitable. A sender that is a list, no-reply or service address
SHALL never be awaited either, and when the sender is a machine nobody is
awaited. The facts SHALL answer neither `next_move` nor `awaited`:
whose turn it is is for the model to read.

#### Scenario: A colleague and a no-reply address
- **WHEN** João writes to Ana Costa, copying the inbox and `no-reply@tickets.example.com`
- **THEN** Ana Costa is the only person who may be awaited, and neither question is answered by the facts

#### Scenario: Nobody to pick
- **WHEN** Marta writes only to the inbox
- **THEN** nobody may be awaited and the `awaited` question is not asked

#### Scenario: A role address
- **WHEN** Sam writes to `info@waterworks.example` and `geral@waterworks.example`
- **THEN** both may be awaited

#### Scenario: A machine sender
- **WHEN** `billing@shop.example` writes to Ana
- **THEN** nobody may be awaited

## ADDED Requirements

### Requirement: What to call a party
The facts SHALL give a party a display name from its headers alone
(`display_name`): the header's display name when it has one that is not a role
word; else the organisation named by the first label of the address's
registrable domain, its words capitalised and no accent added (none for a free
mail provider); else the address. A role word is a display name that, lowercased,
accent-folded and trimmed, equals the address's local part, matches the service
address pattern, or is a generic mailbox name in English, Portuguese, Spanish or
French.

#### Scenario: A shared mailbox shows as the role
- **WHEN** the headers say "Geral" <geral@aguas-ribatejo.example>
- **THEN** the name is "Aguas Ribatejo"

#### Scenario: A person
- **WHEN** the headers say "Ana Costa" <ana@aguas-ribatejo.example>
- **THEN** the name is "Ana Costa"

#### Scenario: A free mail provider
- **WHEN** the headers say "Geral" <geral@gmail.com>
- **THEN** the name is the address

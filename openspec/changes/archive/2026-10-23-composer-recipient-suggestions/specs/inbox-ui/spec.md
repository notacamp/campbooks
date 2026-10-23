## ADDED Requirements

### Requirement: Suggesting people in To, Cc and Bcc
In the composer (New message, Reply, Reply all and Forward's composer), To,
Cc and Bcc SHALL each suggest people as the member types: from two
characters, after typing settles for about 200 ms, the field SHALL ask
`conversations.list_correspondents` with what it holds and show up to 8 of
the people it answers in a list under the field, each with their avatar,
their name and their address (the address alone when there is no name).
People already in To, Cc or Bcc SHALL be left out. The field SHALL follow
the ARIA combobox pattern: focus stays in the input, the first person is
highlighted, ↑ and ↓ move the highlight (wrapping), Enter or Tab add the
highlighted person as a chip, a click or a tap adds the person clicked, and
Esc closes the list without closing the composer. Leaving the field and
adding a person SHALL close the list. While the list is closed, or holds
nobody, Enter, Tab, a comma and leaving the field SHALL add what was typed,
as before. The list SHALL show the keys that work in it, fit the field's
width at 375px and scroll when it is taller than the space it has.

#### Scenario: Picking with the keyboard
- **WHEN** Sam types "mar" in To and Marta Ferreira and Mário Lopes are suggested
- **THEN** Marta is highlighted; ↓ highlights Mário, Enter adds "Mário Lopes" as a chip and the list closes

#### Scenario: Already added
- **WHEN** Marta is already in To and Sam types "mar" in Cc
- **THEN** Cc suggests Mário and not Marta

#### Scenario: A new address
- **WHEN** Sam types "new@studio.example", Esc closes the list, and Sam presses Enter
- **THEN** new@studio.example is added as a chip and the composer stays open

#### Scenario: Esc twice
- **WHEN** the list is open and Sam presses Esc twice
- **THEN** the first Esc closes the list and the second closes the composer, keeping the draft

## ADDED Requirements

### Requirement: Worth a look is not counted as needing the member
Worth a look items SHALL NOT count in the hero's "N things need you" nor in the
Home badge (the sidebar and the tab bar), since they ask nothing of the member;
the group SHALL keep showing its own count. When the only items are Worth a look
ones, the hero SHALL say "Nothing needs you" with the subline "Nothing asks
anything of you. N emails below are worth a look." (singular for one), in the
member's language, and the group SHALL show below it.

#### Scenario: A mixed Home
- **WHEN** Home holds six items that need the member and one Worth a look
- **THEN** the hero says "6 things need you", the badge says 6, and the Worth a look group reads "Worth a look 1"

#### Scenario: Only Worth a look
- **WHEN** Home's only item is a Worth a look one
- **THEN** the hero says "Nothing needs you" and "Nothing asks anything of you. 1 email below is worth a look.", the item is shown, and the badge is empty

### Requirement: Scout noticed keys
Scout noticed's four buttons SHALL show their keys and work from the keyboard
while no confirmation is open: Shift+F (its own verb), Shift+O (Something
else…), Shift+L (Not now) and Shift+D (Don't suggest this). None SHALL share
keys with Home's item keys or any other shortcut.

#### Scenario: Not now by key
- **WHEN** Ana presses Shift+L with a suggestion showing
- **THEN** it is hidden with the usual Undo, as if she had pressed Not now

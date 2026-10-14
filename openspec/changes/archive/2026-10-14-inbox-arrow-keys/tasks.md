# Tasks

## 1. Shortcut system

- [x] 1.1 `alternateKeys` on definitions; conflict check and parsing over every binding
- [x] 1.2 Bind option `alternates` (default true); `liveBindings`; the dispatcher matches live bindings only
- [x] 1.3 Arrow keys left alone in fields, widgets with their own arrow keys, and dialogs
- [x] 1.4 `KbdHint withAlternates` ("J or ↓", spoken in four locales); the `?` sheet uses it

## 2. Inbox

- [x] 2.1 `list.next`/`list.previous` take ↓/↑; the reader binds J/K only
- [x] 2.2 Past the last loaded row, load the next page; scroll the row into view in list and reader
- [x] 2.3 Tests: registry, dispatcher, hint, and the inbox story (arrows move, Enter opens, arrows don't leave an open conversation)

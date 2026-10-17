# Tasks

- [x] SenderFilter: a SearchField over CheckCards; local filtering while
      typing, `list_senders?q=` from two characters (debounced); ticked
      senders stay; a typed address or domain becomes its own card;
      free-mail refused with why; Enter ticks the first match, Esc clears.
- [x] Questions uses it on the "always" view (with the suggested senders)
      and the "never" view (no suggestions); names of found senders reach
      the rule sentences.
- [x] i18n in en, pt-PT, es-ES and fr.
- [x] Tests: finding by name beyond the first ten, ticked senders kept,
      typed address and free-mail, Enter and Esc in the box.
- [x] Storybook screen with the filter in use (light and dark, axe).
- [x] Recording: a `find-senders` Storybook scenario finds someone by name.

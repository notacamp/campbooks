# Tasks

## 1. Server

- [ ] 1.1 `identity.list_ways_in`: `mailbox_providers` (configured upstreams with mailbox scopes), with a test
- [ ] 1.2 OpenAPI regenerated; client types regenerated

## 2. Design system

- [ ] 2.1 `ProviderMark`, `HealthText`, `PhoneSubnav`, `ListRow` selected state, with stories and tests
- [ ] 2.2 `settings` shortcut scope

## 3. Clients

- [ ] 3.1 `identity`: `useMailboxConsent` (start connect/reconnect, step-up, refusals)
- [ ] 3.2 `connectors`: Mailboxes page (list, health, connect, reconnect, disconnect, results, keys), disconnect mutation, route
- [ ] 3.3 App shell: consent port wiring, settings sections (sidebar + phone chips), Home's button, mail section link, held offer link
- [ ] 3.4 en / pt-PT / es-ES / fr; tests; screen stories for every state, both accents, dark
- [ ] 3.5 Fake provider: `POST /fake/expire` (dev-only) to show a mailbox that needs reconnecting; recording scenario `mailboxes`

## 4. Wrap-up

- [ ] 4.1 `openspec/decisions-needed.md`: mail after a disconnect
- [ ] 4.2 CHANGELOG updated
- [ ] 4.3 Gates green; UI verified at 375px and desktop; recording

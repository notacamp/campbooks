# Tasks

## 1. Server

- [x] 1.1 `Deps::offers`, `ConnectionView.offers`, filled wherever a view is built
- [x] 1.2 `identity.start_link` `capabilities` (`calendar` only), merged into the ticket
- [x] 1.3 Tests: Google offers calendar, Zoho only while its flag is on, Microsoft and IMAP never; a reconnect that names calendar asks for its scopes

## 2. Client

- [x] 2.1 Settings → Calendar lists connections by `offers`, asks for `calendar`, explains each refusal (step-up banner with Sign in again)
- [x] 2.2 `ConsentStart` reconnect carries `capabilities`; `identity` passes it to `start_link`
- [x] 2.3 en, pt-PT, es-ES, fr strings; stories and tests for the step-up, Zoho offered and not offered

## 3. Last

- [x] 3.1 OpenAPI and client types updated
- [x] 3.2 Tracking plan unchanged
- [x] 3.3 Changelog entry files (`changelog/unreleased/fixed/373-calendar-access-sign-in-again.md`, `added/373-zoho-calendar-offered.md`)
- [x] 3.4 Gates green, UI verified at 375px and desktop (recording)

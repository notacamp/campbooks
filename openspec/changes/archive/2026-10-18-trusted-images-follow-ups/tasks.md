# Tasks

## 1. Server

- [x] 1.1 Free mail list widened (342), by registrable domain; one source of truth
- [x] 1.2 Body: `images_offer`, `images_domain`, `images_held`
- [x] 1.3 A sender already trusted is refused (409 `images_already_trusted`), on create and on change

## 2. Client

- [x] 2.1 The client's list checked against the server's; registrable matching
- [x] 2.2 Reader: offer, ⇧A and the domain choice follow the server; held reasons; shared inbox note; Scout's check in the confirmation
- [x] 2.3 Strings in en, pt-PT, es-ES and fr

## 3. Tests

- [x] 3.1 Engine: lookalike and suffix domains; free mail subdomains refused by their registrable domain
- [x] 3.2 Assessment unit tests; body: offers, held reasons, duplicates refused, a stored `suspicious = true` blocking a trusted sender
- [x] 3.3 Reader: held note without offer or ⇧A, address only without a domain, shared inbox note

## 4. Docs

- [x] 4.1 Recording scenario `trusted-images` updated
- [x] 4.2 CHANGELOG
- [x] 4.3 Gates green in CI

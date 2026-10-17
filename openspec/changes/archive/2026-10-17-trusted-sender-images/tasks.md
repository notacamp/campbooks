# Tasks

## 1. Server

- [x] 1.1 Engine: `show_images` action (its own slot), validated as one sender alone; `shows_images` with the authenticated, suspicious and failed-checks guards
- [x] 1.2 Facts: `sender_authenticated` (DMARC pass or aligned DKIM pass; no results is no pass)
- [x] 1.3 Conversations: `RuleAction::ShowImages`; a rule needs mail from its sender in the inbox; no apply run
- [x] 1.4 Message body: images kept for a trusted, authenticated sender; `images_trusted`; tracking pixels still removed
- [x] 1.5 OpenAPI and client types

## 2. Client

- [x] 2.1 Reader: Always show images from <sender> (⇧A), the trusted note
- [x] 2.2 Confirmation: address or domain, images summary, no apply; removal question in Scout's memory
- [x] 2.3 Strings in en, pt-PT, es-ES and fr

## 3. Tests

- [x] 3.1 Engine and facts: validation, guards, spoofed, failed DMARC, no results, aligned DKIM
- [x] 3.2 Body: trusted shows images without the pixel; spoofed, failed DMARC and no results stay held back; removal holds them back; unseen sender refused; no run
- [x] 3.3 Reader: confirm and save (domain), the note, Shift+A, none for a reader

## 4. Docs

- [x] 4.1 Demo mailbox and recording scenario `trusted-images`
- [x] 4.2 CHANGELOG
- [x] 4.3 Gates green in CI

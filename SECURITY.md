# Security Policy

We take the security of Campbooks seriously — it handles people's email, OAuth
tokens, and documents. Thank you for helping keep it and its users safe.

## Supported versions

Campbooks is delivered as a rolling source-available release. Security fixes land
on `main` (the latest release) and are not back-ported to older tags. If you
self-host, **run the latest release** to stay covered.

## Reporting a vulnerability

**Please do not report security issues in public GitHub issues, pull requests,
or discussions.** Disclose privately instead, by either:

- Opening a private advisory via GitHub →
  [**Security → Report a vulnerability**](https://github.com/notacamp/campbooks/security/advisories/new), or
- Emailing **security@not-a-camp.com**.

Please include enough to reproduce:

- A description of the issue and its impact.
- Steps to reproduce, or a proof of concept.
- Affected version (from `/up`, the Settings sidebar, or the `VERSION` file) and
  whether you're on the hosted cloud or self-hosting.
- Any relevant logs or screenshots — **with secrets redacted**.

## What to expect

We're a small team and respond on a best-effort basis. We aim to acknowledge a
report within a few business days, keep you updated as we investigate, and credit
you (if you'd like) once a fix ships. Please give us a reasonable window to
release a fix before any public disclosure.

## Scope

**In scope:** the application code in this repository — authentication and
sessions, the OAuth/mailbox/calendar integrations, the public REST API
(`/api/v1`) and webhooks, the workflow engine, and data-access permission gates.

**Out of scope:** vulnerabilities that require a misconfigured self-hosted
deployment (e.g. running without TLS, weak `.env` secrets, an exposed database),
issues in third-party providers (Google, Microsoft, Zoho, Notion, …), and
findings from automated scanners without a demonstrated, realistic impact.

# 0003 — One React/TypeScript UI, shipped everywhere with Tauri 2

**Status:** Accepted · 2026-10-12

## Context
Targets: web, native desktop (macOS, Windows, Linux), tablet and mobile (iOS,
Android). Criteria, in order: (1) ease of shipping to every platform from one
codebase; (2) security — as few tokens as possible exposed to a browser
runtime.

## Decision
- **One UI codebase** in React + TypeScript (Vite, TanStack Router/Query,
  i18next), at `clients/app`.
- **Tauri 2** wraps it for desktop and mobile. The Tauri shell is Rust: it owns
  the HTTP client and the OAuth tokens (stored in the OS keychain/keystore), and
  the webview calls the API through a Tauri command, so JavaScript never holds a
  token.
- **Web** is the same build served by the server; it talks to a same-origin BFF
  with an httpOnly cookie (ADR 0004).
- `src/lib/api` hides the transport difference; modules are platform-agnostic.

## Consequences
- One design system, one test suite, one i18n catalogue for every platform.
- Platform-specific capabilities (notifications, share sheets, file pickers)
  go through Tauri plugins behind `src/lib/platform`.
- Mobile store builds need code signing and CI runners with Xcode/Android SDK;
  this is a later task.
- HTML email rendering stays in a sandboxed iframe on every platform.

## Alternatives considered
- All-Rust UI (Dioxus/Leptos) — one language, but a thin component ecosystem
  for rich email, editors and calendar grids.
- Flutter — excellent mobile, but canvas-rendered web is poor for HTML email
  and accessibility, and adds Dart.
- React Native/Expo — strong mobile, weak desktop.
- Capacitor — mobile only; desktop would need Electron alongside it.

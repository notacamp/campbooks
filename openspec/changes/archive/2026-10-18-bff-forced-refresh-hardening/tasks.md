## 1. Fixes

- [x] 1.1 L1: drop the refresh's transaction before ending a session whose stored token does not open
- [x] 1.2 L2: hold bodies of a known length up to 1 MiB and send them again after a forced refresh
- [x] 1.3 The `/bff/admin` note

## 2. Tests

- [x] 2.1 A small `POST` is sent again with its body; a larger one is not
- [x] 2.2 Two requests refused together share one refresh
- [x] 2.3 The identity provider down: 503 `idp_unavailable`, the session kept

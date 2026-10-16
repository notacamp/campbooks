## 1. Authentication
- [x] 1.1 `dkim=pass` counts only when no `dmarc` result refuses it
- [x] 1.2 `Authentication-Results` over 8 KiB counts as authentication failed
- [x] 1.3 Registrable domain and public suffix from the Public Suffix List (`psl`); bare suffix From authenticates nothing
## 2. Needs
- [x] 2.1 A first need is not stored for a message mid-move (`FOR SHARE SKIP LOCKED`, never waits)
- [x] 2.2 Tests: leaving spam, trash round trip with a confirmation and a Done, the race itself
## 3. Docs
- [x] 3.1 `CHANGELOG.md`
- [x] 3.2 Archive the change

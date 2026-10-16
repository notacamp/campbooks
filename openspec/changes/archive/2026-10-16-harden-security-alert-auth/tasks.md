## 1. Authentication
- [x] 1.1 `dkim=pass` counts only when no `dmarc` result says anything but `pass`
- [x] 1.2 `Authentication-Results` over 8 KiB reads as no results
- [x] 1.3 Registrable domain and public suffix from the Public Suffix List (`psl`); bare suffix From authenticates nothing
- [x] 1.4 Platform alerts need the platform's whole display name
## 2. Needs
- [x] 2.1 Refresh locks its source messages `FOR SHARE` before the needs row
- [x] 2.2 Tests: leaving spam, trash round trip with a confirmation and a Done, the race itself
## 3. Docs
- [x] 3.1 `CHANGELOG.md`
- [x] 3.2 Archive the change

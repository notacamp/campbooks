- **Dependency audit clean again (RUSTSEC-2026-0436).** The spreadsheet reader behind the
  tracking-plan tool moves to calamine 0.38.0, which fixes a crash and runaway memory use on
  malformed `.xls`/`.xlsb` files. That tool only reads our own workbook, so the app and
  self-hosted servers were never exposed. Nothing changes for users.

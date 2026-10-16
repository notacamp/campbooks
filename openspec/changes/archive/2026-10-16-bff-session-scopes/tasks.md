## 1. Server

- [x] 1.1 Migration: `bff.sessions.scopes` (nullable text)
- [x] 1.2 Record the scopes asked for at sign-in
- [x] 1.3 Session lookup ends a session that did not ask for every current scope (refresh token revoked, provider session kept)
- [x] 1.4 Tests: a session missing a newer scope ends and the next sign-in stays; a session with no recorded scopes ends; set comparison

## 2. Finish

- [x] 2.1 `.sqlx` regenerated, CHANGELOG
- [x] 2.2 Gates green; terminal recording of the tests
- [x] 2.3 Archive

## MODIFIED Requirements

### Requirement: The web sign-in's tables are reached only through the BFF's functions
The runtime role SHALL hold no privilege on `bff.sessions`,
`bff.login_transactions` and `bff.retired_tokens`, which SHALL enable
row-level security with one policy, admitting only the functions' owner
`campbooks_definer`. It SHALL reach them only through the
`SECURITY DEFINER` functions of schema `bff`, one per operation of the web
BFF: `create_session`, `find_session`, `touch_session`,
`mark_session_delivered`, `rotate_session`, `reissue_session`,
`retired_session`, `end_session`, `lock_session_tokens`,
`store_session_tokens`, `end_account_sessions`, `prune_sessions`,
`start_login`, `take_login` and `prune_logins`. Each SHALL run with a pinned
`search_path`, be executable by the runtime role only, and act on the one
session named by the SHA-256 of a token its cookie presented, the one
sign-in named by its state's SHA-256, or the sessions of one account (or of
one identity-provider session); none SHALL answer a session's token hashes
or list sessions. Only the two prunes SHALL reach across accounts, deleting
only rows ended by the database's clock, at most 10 000 per call, each row
checked again as it is deleted. The functions creating rows SHALL refuse
token, state and browser hashes that are not 32 bytes, missing sealed
values, a sign-in longer than an hour, a session longer than 366 days and
deadlines out of order, and a landing the handler would refuse: longer than
2 048 characters, not starting with exactly one `/`, holding anything but
visible ASCII, a backslash, `"`, `'`, `<`, `>` or a backtick, or naming the
BFF's own paths. `touch_session` SHALL refuse a missing time or idle
deadline, or a deadline not after the use, and `store_session_tokens` a
missing or empty sealed access or refresh token, an empty ID token or a
missing deadline; a missing ID token keeps the stored one.

#### Scenario: Direct access as the runtime role
- **WHEN** a transaction of the runtime role selects, updates, deletes, inserts, truncates or locks any of the three tables while a session, a sign-in and a retired token exist
- **THEN** each statement is refused (permission denied), and the session still works

#### Scenario: Another account's session
- **WHEN** two people are signed in, and the runtime role calls the functions with the first person's token hash, an unknown hash, an empty or `NULL` hash, and ends the first person's account sessions
- **THEN** it finds only the first person's session, nothing for the others, and only the first person is signed out

#### Scenario: Everything still works through the functions
- **WHEN** a person signs in, the cookie rotates and is reissued, the access token is refreshed, the person signs out, signs out everywhere or is suspended, and the prune runs, all as the runtime role
- **THEN** each behaves as before, and the prune deletes ended sessions and expired sign-ins only

#### Scenario: Input the BFF never sends
- **WHEN** the runtime role starts a sign-in landing on `//evil.example`, `/\evil.example`, a path with a space or a tab, `/"><script>`, `/bff/logout` or a path of 2 049 characters, touches a session with no time, no deadline or a deadline equal to the use, stores tokens with none or an empty one, or creates a session of 367 days or whose idle deadline is its start
- **THEN** each call fails as invalid input and nothing changes, and a sign-in landing on `/inbox?view=all#top` is accepted

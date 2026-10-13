## MODIFIED Requirements

### Requirement: Sign-in screen
`/sign-in?request=<id>` SHALL read `GET /auth/requests/{id}` and show the
brand, one line about Campbooks and one action per way in the response lists
that the client knows (`google`, `microsoft`, `zoho`, `passkey`), in the
listed order, ignoring unknown ones. Provider actions SHALL navigate to
`/auth/start/<provider>?request=<id>` and SHALL have the keys `1`, `2` and
`3` by position. The passkey action SHALL have `P`, SHALL be shown only in a
browser with passkeys, and SHALL run the passkey ceremony in place
(`/auth/passkey/sign-in/options`, the browser's dialog,
`/auth/passkey/sign-in`), then follow the answer's redirect. Where the
browser offers passkeys in a field's autofill, the screen SHALL also show a
"Saved passkeys" field (`autocomplete="username webauthn"`) whose autofill
ceremony starts when the screen opens. A passkey that is not accepted, a
ceremony that expired or was cancelled, and too many tries SHALL be explained
in place. When the request says passkey sign-up is open, the screen SHALL
link to `/sign-in/passkey?request=<id>`, a one-question screen (what to call
you; Create with Enter, Sign in another way with Escape) that creates the
account with a new passkey and explains a refusal (`signup_invite_only`, a
browser without passkeys) on its own page. With no way in listed,
the screen SHALL say that sign-in is not set up and to ask the instance's
administrator. A request the server no longer knows (404) SHALL be
explained, with an action that starts again. Without `request`, the screen
SHALL remember `return_to` for the tab and navigate to
`/bff/login?return_to=<return_to>` (`/` when absent or not a same-origin
path); a browser already signed in SHALL go to `return_to` instead. When the
remembered `return_to` is an invitation link, the screen SHALL use the
invitation copy.

#### Scenario: Google only
- **WHEN** `/sign-in?request=r1` loads and the request lists `["google"]`
- **THEN** one "Continue with Google" link to `/auth/start/google?request=r1` is shown with the key hint `1`, and pressing `1` follows it

#### Scenario: Every way in
- **WHEN** the request lists `["google", "microsoft", "zoho", "passkey"]`
- **THEN** Google, Microsoft and Zoho have keys `1`, `2` and `3`, and "Use a passkey" has `P`

#### Scenario: Sign in with a passkey from the screen
- **WHEN** a browser with passkeys presses `P` on `/sign-in?request=r1` and approves a passkey
- **THEN** the screen posts the assertion and follows the returned redirect to the web client

#### Scenario: Passkey cancelled
- **WHEN** the person closes the browser's passkey dialog
- **THEN** the screen says no passkey was used and "Use a passkey" can be pressed again

#### Scenario: Start from a deep link
- **WHEN** a signed-out browser opens `/sign-in?return_to=%2Fw%2F1234`
- **THEN** it navigates to `/bff/login?return_to=%2Fw%2F1234`

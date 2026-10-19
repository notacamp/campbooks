## MODIFIED Requirements

### Requirement: HTML mail on the desktop app
The desktop app SHALL render HTML mail as the web app does: the same
sandboxed frame, its own policy and the same sanitised body, styled and with
its `data:` and inline images, remote images held back until Show images or a
trust rule. The desktop page's policy SHALL add only `style-src-attr
'unsafe-inline'` and `img-src 'self' data: blob: https:` to `default-src
'self'`, keep `script-src 'self'` and `style-src 'self'` (never
`'unsafe-inline'` on either), `object-src 'none'`, `base-uri 'none'`,
`form-action 'none'` and `frame-ancestors 'none'`, and leave frames under
`default-src 'self'`; plain `http:` images stay blocked on desktop. Nothing
in the mail frame SHALL run, so Tauri's IPC is out of its reach. A link that
asks for a new window SHALL never open a webview, and the main webview SHALL
never leave the app's own pages: `tauri://localhost`, and
`http(s)://tauri.localhost` only on Windows and Android, where the app is
served from it (elsewhere it is the machine itself), plus the dev server in a
debug build only. A web link opens in the system's browser, a mail link in
the mail app with only its address, copies, subject and body and no
fragment, each value decoded and encoded again with everything but
unreserved characters as `%XX` (a space as `%20`, never `+`; a plus as `%2B`;
a line break, `;`, `?`, `=`, `/`, `&` or `#` encoded too, so no field hides in
a value), and only when it has no authority (`mailto://…`) and its address
part and its `to`, `cc` and `bcc` fields are nothing but addresses (none, or
comma-separated `local@domain`s, valid UTF-8, not starting with `/`, with no
control character, Unicode blank, invisible character such as a bidi
override, a zero-width space or a variation selector, or delimiter such as
CR, LF, `&`, `;`, `?` or `\`, even encoded; other Unicode, as in an
internationalised address, is kept); a subject or body holding `&` or `?`
once decoded SHALL be dropped, since a mail app that decodes before it splits
would read another field there, and so SHALL a subject that is not valid
UTF-8 or holds a control character (CR and LF included), a line or paragraph
separator (U+2028, U+2029) or an invisible character, and a body that is not
valid UTF-8 or holds a control character other than CR, LF and tab, since a
mail app that decodes first would start another header there; anything else nowhere; no remote URL SHALL be granted
IPC. CI SHALL check the frame in WebKit under the desktop and web policies
read from their definitions, with the app's own sandbox and referrer policy,
and a control proving the check would see a script run.

#### Scenario: A newsletter on desktop
- **WHEN** the member opens the #160 newsletter on the desktop app
- **THEN** it shows its inline styles and placeholders, nothing remote loads, and after Show images its https images load with no tracking pixel

#### Scenario: Hostile markup
- **WHEN** a body carries a script, an `onerror`, an `<svg onload>`, a `javascript:` link, a form, a meta refresh, `@import` or a remote style sheet
- **THEN** nothing runs, posts, navigates or loads under the desktop policy

#### Scenario: A link in a mail
- **WHEN** the member clicks a web link in a mail on the desktop app
- **THEN** it opens in the system browser, and no Campbooks window shows the remote page

#### Scenario: A look-alike on the machine
- **WHEN** a link to `http://tauri.localhost/` reaches the main window on macOS or Linux
- **THEN** the window stays where it is and the link opens in the system browser

#### Scenario: A mail link with spaces
- **WHEN** the member clicks `mailto:rita@example.com?subject=Tax%20dates&attach=/etc/passwd`
- **THEN** the mail app gets `mailto:rita@example.com?subject=Tax%20dates`

#### Scenario: A field hidden in a value or the fragment
- **WHEN** the member clicks `mailto:rita@example.com?subject=hi#&attach=/etc/passwd` or `mailto:rita@example.com?subject=x;attach=/etc/passwd`
- **THEN** the mail app gets `mailto:rita@example.com?subject=hi`, or `mailto:rita@example.com?subject=x%3Battach%3D%2Fetc%2Fpasswd`

#### Scenario: A header hidden in the address
- **WHEN** the member clicks `mailto:rita@example.com%0D%0Abcc:evil@example.org` or `mailto:rita@example.com%26attach=/etc/passwd`
- **THEN** nothing opens, while `mailto:rita@example.com,joao@example.com` opens in the mail app

#### Scenario: More addresses, or a field hidden in the body
- **WHEN** the member clicks `mailto:?to=rita@example.com%0D%0Abcc:evil@example.org`, or `mailto:rita@example.com?body=x%26attach%3D/etc/passwd`
- **THEN** the first opens nowhere, and the second opens with no body

#### Scenario: An authority or an invisible character
- **WHEN** the member clicks `mailto://x%0D%0Abcc:evil@example.org`, or an address holding U+202E, U+200B, U+0085 or a byte that is not UTF-8
- **THEN** nothing opens, while `mailto:ana@exämple.example` opens in the mail app

#### Scenario: A header hidden in the subject
- **WHEN** a mail link reads `mailto:rita@x.example?subject=Hi%0D%0ABcc:%20evil@x.example&body=Thanks`, or its body holds a NUL
- **THEN** the subject (or the body) is left out and the rest opens; a body over several lines, with tabs, is kept as written

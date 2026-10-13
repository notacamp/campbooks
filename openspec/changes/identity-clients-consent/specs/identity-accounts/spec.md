## ADDED Requirements

### Requirement: Consent screen
The web client SHALL have a consent screen at `/consent?request=` in the
centred one-question layout of the sign-in screens. It SHALL name the app
and whether Campbooks or a workspace admin vouches for it ("Unverified"
otherwise), say whom the person is signed in as, list what the app will be
able to do in plain sentences, ask which workspace it may use (only its own
for a workspace's app; "all your workspaces" for Campbooks' own apps), say
where the browser goes next, warn when the app is on this device, and say
whether the answer will be remembered. Allow (⌘↵) and Deny (Esc) SHALL send
the answer and follow the redirect. `step_up_required` SHALL offer "Sign in
again"; a request no longer waiting SHALL be explained. It SHALL be in en,
pt-PT, es-ES and fr, keyboard-operable with visible hints, and usable at
375px.

#### Scenario: An agent on this device
- **WHEN** the screen shows a dynamically registered agent with a loopback redirect
- **THEN** it says "Unverified", warns to allow it only if the person just started it, and Allow stays disabled until a workspace is picked

### Requirement: Apps and integrations screen
Settings SHALL have an Apps and integrations page, linked from the account
menu, listing the person's connected apps (where each works, what it can do,
when it was allowed and last used) with Revoke behind an inline
confirmation, and, for owners and admins, the open workspace's own apps with
New app (name, server-to-server or sign-in app, read or read and change, a
return address for a sign-in app), Rotate secret and Remove, each confirmed
in place. A secret SHALL be shown once with Copy and never again. Members
SHALL see that only owners and admins manage the workspace's apps. Creating
or rotating with a stale sign-in SHALL offer "Sign in again". It SHALL be in
en, pt-PT, es-ES and fr, keyboard-operable with visible hints (New app:
⇧N), and usable at 375px.

#### Scenario: Secret shown once
- **WHEN** an owner creates a server-to-server app
- **THEN** the page shows its client id and secret with Copy, and after Done the secret is gone from the page

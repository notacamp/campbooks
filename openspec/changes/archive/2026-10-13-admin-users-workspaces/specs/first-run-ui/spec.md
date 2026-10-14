## MODIFIED Requirements

### Requirement: Sign-in errors explained
The sign-in screen SHALL explain each `error` the identity module sends it
(`cancelled_at_provider`, `invalid_state`, `provider_unavailable`,
`login_failed`, `account_suspended`, `way_in_belongs_to_another_account`,
`signup_invite_only`)
and any other code generically, each saying what happened and what to do,
above the ways in when the request is still waiting, or with an action that
starts again when it is not.

#### Scenario: Cancelled at Google
- **WHEN** `/sign-in?error=cancelled_at_provider&request=r1` loads
- **THEN** the screen says the sign-in was cancelled on Google's screen and nothing was created, and the ways in are shown again

#### Scenario: State expired
- **WHEN** `/sign-in?error=invalid_state` loads
- **THEN** the screen says the sign-in took too long or was opened in another browser, and "Start again" navigates to `/bff/login` with the remembered `return_to`

#### Scenario: Suspended account
- **WHEN** `/sign-in?error=account_suspended` loads
- **THEN** the screen says the account is suspended by whoever runs the instance and to ask them to restore it

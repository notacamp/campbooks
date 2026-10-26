## ADDED Requirements

### Requirement: Instance apps' changes are tracked
Every change the server's `campbooks-server admin apps` command makes to an
instance app SHALL be tracked as an instance-wide event of the tracking
plan, in the transaction of the change, with actor `system` and the
command's name (`admin`): an app registered
(`identity.instance_app_registered`, with the client's snapshot), given a
new secret (`identity.instance_app_secret_rotated`, the snapshot
afterwards) and removed (`identity.instance_app_removed`, with its client
id and how many consents and refresh-token families were revoked). A
rotation or removal of an unknown client id SHALL track nothing; listing
SHALL track nothing. The events SHALL never carry the app's name, secret,
scopes or URIs. The client kind `instance` and the `app_` client ids SHALL
be valid wherever the plan names a client's kind or Campbooks-minted id.

#### Scenario: An operator registers, rotates and removes an app
- **WHEN** an operator registers Applicamp, rotates its secret, removes it, and then rotates and removes it again
- **THEN** one event of each kind is written with no workspace and actor `system` `admin`, the registration's snapshot has kind `instance` and `confidential` true, the removal counts the consents and sign-ins it ended, and none holds the name or the secret

#### Scenario: An instance app's sign-in
- **WHEN** a person signs in to an instance app and it redeems the code
- **THEN** `identity.tokens_issued` names it with kind `instance` and its `app_` client id

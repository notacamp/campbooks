## ADDED Requirements

### Requirement: Unverified addresses are not shown as members'
The workspaces module SHALL record whether the identity provider verified a
user's address at their last sign-in. A member's address SHALL be listed
(`workspaces.list_members`, `workspaces.change_role`) only when verified, and
inviting an address SHALL treat it as a member's only when a member verified
it.

#### Scenario: A claimed address
- **WHEN** a person whose provider did not verify `ceo@example.com` creates a workspace and its members are listed
- **THEN** their entry has no address, and after a sign-in where the provider verifies it the address is listed

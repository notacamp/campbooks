## REMOVED Requirements

### Requirement: Security alerts are a to-do
**Reason**: #499 removed the content rules from the decide pass, this one included. A decision model reads the message and header facts may only veto (ADR 0018); nothing implements this requirement any more.
**Migration**: None. Security alerts are judged by the decision model like any other mail; the authentication facts it is handed are specified in `ai-decide-facts`.

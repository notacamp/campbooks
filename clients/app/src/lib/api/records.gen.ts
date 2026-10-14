/**
 * Generated from openapi/openapi.json by scripts/records-gen.ts. Do not edit.
 *
 * For each record type, the read operations that return it: a change to a
 * record of that type invalidates the queries keyed by those operationIds.
 */
export const recordReaders: Readonly<Record<string, readonly string[]>> = {
  "attention.item": ["attention.list"],
  "connectors.connection": ["connectors.get_connection", "connectors.list_connections"],
  "conversations.conversation": [
    "conversations.get",
    "conversations.list",
    "conversations.list_streams",
  ],
  "conversations.draft": ["conversations.get_draft", "conversations.list_drafts"],
  "conversations.inbox": ["conversations.list_inboxes"],
  "conversations.message": [
    "conversations.get",
    "conversations.get_attachment",
    "conversations.get_message_body",
  ],
  "identity.authorization_code": ["identity.authorize"],
  "identity.authorize_request": [
    "identity.authorize",
    "identity.get_consent_request",
    "identity.get_sign_in_request",
  ],
  "identity.client": ["identity.list_clients"],
  "identity.consent": ["identity.list_consents"],
  "identity.held_grant": ["identity.list_held_grants"],
  "identity.passkey": ["identity.list_passkeys"],
  "identity.pending_sign_up": ["identity.finish_sign_in", "identity.get_new_account"],
  "identity.provider_identity": ["identity.list_ways_in"],
  "identity.session": ["identity.end_session", "identity.finish_sign_in"],
  "identity.signing_key": ["identity.jwks"],
  "identity.upstream_flow": ["identity.finish_sign_in", "identity.start_sign_in"],
  "search.result": ["search.query"],
  "workspaces.invitation": ["workspaces.list_invitations"],
  "workspaces.member": ["workspaces.list_members"],
  "workspaces.security_event": ["workspaces.list_security_events"],
  "workspaces.workspace": ["workspaces.get", "workspaces.list"],
};

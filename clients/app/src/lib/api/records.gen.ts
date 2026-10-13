/**
 * Generated from openapi/openapi.json by scripts/records-gen.ts. Do not edit.
 *
 * For each record type, the read operations that return it: a change to a
 * record of that type invalidates the queries keyed by those operationIds.
 */
export const recordReaders: Readonly<Record<string, readonly string[]>> = {
  "connectors.connection": ["connectors.get_connection", "connectors.list_connections"],
  "conversations.conversation": ["conversations.get", "conversations.list"],
  "conversations.inbox": ["conversations.list_inboxes"],
  "conversations.message": ["conversations.get", "conversations.get_message_body"],
  "workspaces.invitation": ["workspaces.list_invitations"],
  "workspaces.member": ["workspaces.list_members"],
  "workspaces.security_event": ["workspaces.list_security_events"],
  "workspaces.workspace": ["workspaces.get", "workspaces.list"],
};

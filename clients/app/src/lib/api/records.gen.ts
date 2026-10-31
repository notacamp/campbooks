/**
 * Generated from openapi/openapi.json by scripts/records-gen.ts. Do not edit.
 *
 * For each record type, the read operations that return it: a change to a
 * record of that type invalidates the queries keyed by those operationIds.
 */
export const recordReaders: Readonly<Record<string, readonly string[]>> = {
  "assistant.chat": ["assistant.get_chat", "assistant.list_chats"],
  "assistant.profile": ["assistant.get_profile"],
  "attention.item": ["attention.list"],
  "connectors.connection": ["connectors.discover_imap", "connectors.get_connection", "connectors.list_connections", "connectors.list_notion_data_sources", "connectors.list_notion_pages"],
  "conversations.conversation": ["conversations.get", "conversations.get_history", "conversations.list", "conversations.list_streams", "conversations.suggest_rules"],
  "conversations.draft": ["conversations.get_draft", "conversations.list_drafts"],
  "conversations.inbox": ["conversations.get_signature", "conversations.list_correspondents", "conversations.list_inboxes", "conversations.list_labels", "conversations.list_senders"],
  "conversations.message": ["conversations.get", "conversations.get_attachment", "conversations.get_history", "conversations.get_message_body", "conversations.get_message_read"],
  "conversations.person_note": ["conversations.list_person_notes"],
  "conversations.rule": ["conversations.get", "conversations.get_history", "conversations.get_message_body", "conversations.get_rule", "conversations.get_rule_notice", "conversations.list", "conversations.list_rule_notices", "conversations.list_rules", "conversations.preview_rule", "conversations.suggest_rules"],
  "identity.authorization_code": ["identity.authorize"],
  "identity.authorize_request": ["identity.authorize", "identity.get_consent_request", "identity.get_sign_in_request"],
  "identity.client": ["identity.list_clients"],
  "identity.consent": ["identity.list_consents"],
  "identity.held_grant": ["identity.list_held_grants"],
  "identity.passkey": ["identity.list_passkeys"],
  "identity.pending_sign_up": ["identity.finish_sign_in", "identity.get_new_account"],
  "identity.provider_identity": ["identity.list_ways_in"],
  "identity.session": ["identity.end_session", "identity.finish_sign_in"],
  "identity.signing_key": ["identity.jwks"],
  "identity.upstream_flow": ["identity.finish_sign_in", "identity.start_sign_in"],
  "pages.page": ["pages.backlinks", "pages.get", "pages.get_file", "pages.get_sharing", "pages.get_table", "pages.get_table_sync", "pages.list", "pages.list_notion_links", "pages.query_table", "pages.read_text"],
  "schedule.calendar": ["schedule.get_settings", "schedule.list_calendars"],
  "schedule.event": ["schedule.free_slots", "schedule.get_event", "schedule.list_events"],
  "search.result": ["search.query"],
  "tasks.task": ["tasks.get", "tasks.list"],
  "workspaces.invitation": ["workspaces.list_invitations"],
  "workspaces.member": ["workspaces.list_members"],
  "workspaces.security_event": ["workspaces.list_security_events"],
  "workspaces.workspace": ["workspaces.get", "workspaces.list"],
};

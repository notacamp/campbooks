# frozen_string_literal: true

class AddUuidColumns < ActiveRecord::Migration[8.1]
  def up
    add_column :account_exports, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :agent_messages, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :agent_threads, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :ai_adapters, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :ai_configurations, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :audit_events, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :beta_codes, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :bug_reports, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :calendar_account_users, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :calendar_accounts, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :calendar_events, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :calendar_sync_logs, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :calendar_webhook_channels, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :calendars, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :connections, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :contact_email_aliases, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :contact_tags, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :contacts, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :devices, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :document_drive_uploads, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :document_email_messages, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :document_templates, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :document_types, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :documents, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :drive_folder_mappings, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :email_account_signatures, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :email_account_users, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :email_accounts, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :email_folders, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :email_message_tags, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :email_messages, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :email_scan_logs, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :email_threads, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :events, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :exports, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :feed_items, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :folder_memberships, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :google_drive_accounts, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :google_drive_configs, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :identities, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :invitations, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :mail_folders, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :mfa_email_challenges, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :notification_preferences, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :notifications, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :notion_database_mappings, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :notion_integrations, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :notion_pages, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :oauth_applications, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :organization_memberships, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :organizations, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :people, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :pipeline_memberships, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :pipeline_stages, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :pipelines, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :recovery_codes, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :reminders, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :scheduled_emails, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :search_chunks, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :search_records, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :search_tag_embeddings, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :sessions, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :signatures, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :signup_requests, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :skim_decisions, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :tags, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :templates, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :thread_follows, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :users, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :webauthn_credentials, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :workflow_execution_steps, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :workflow_executions, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :workflow_steps, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :workflows, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :workspaces, :uuid_col, :uuid, default: "gen_random_uuid()"
    add_column :zoho_drive_accounts, :uuid_col, :uuid, default: "gen_random_uuid()"

    execute "UPDATE account_exports SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE agent_messages SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE agent_threads SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE ai_adapters SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE ai_configurations SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE audit_events SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE beta_codes SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE bug_reports SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE calendar_account_users SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE calendar_accounts SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE calendar_events SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE calendar_sync_logs SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE calendar_webhook_channels SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE calendars SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE connections SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE contact_email_aliases SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE contact_tags SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE contacts SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE devices SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE document_drive_uploads SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE document_email_messages SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE document_templates SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE document_types SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE documents SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE drive_folder_mappings SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE email_account_signatures SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE email_account_users SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE email_accounts SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE email_folders SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE email_message_tags SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE email_messages SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE email_scan_logs SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE email_threads SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE events SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE exports SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE feed_items SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE folder_memberships SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE google_drive_accounts SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE google_drive_configs SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE identities SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE invitations SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE mail_folders SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE mfa_email_challenges SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE notification_preferences SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE notifications SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE notion_database_mappings SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE notion_integrations SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE notion_pages SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE oauth_applications SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE organization_memberships SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE organizations SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE people SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE pipeline_memberships SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE pipeline_stages SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE pipelines SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE recovery_codes SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE reminders SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE scheduled_emails SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE search_chunks SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE search_records SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE search_tag_embeddings SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE sessions SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE signatures SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE signup_requests SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE skim_decisions SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE tags SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE templates SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE thread_follows SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE users SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE webauthn_credentials SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE workflow_execution_steps SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE workflow_executions SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE workflow_steps SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE workflows SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE workspaces SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"
    execute "UPDATE zoho_drive_accounts SET uuid_col = gen_random_uuid() WHERE uuid_col IS NULL"

    change_column_null :account_exports, :uuid_col, false
    change_column_null :agent_messages, :uuid_col, false
    change_column_null :agent_threads, :uuid_col, false
    change_column_null :ai_adapters, :uuid_col, false
    change_column_null :ai_configurations, :uuid_col, false
    change_column_null :audit_events, :uuid_col, false
    change_column_null :beta_codes, :uuid_col, false
    change_column_null :bug_reports, :uuid_col, false
    change_column_null :calendar_account_users, :uuid_col, false
    change_column_null :calendar_accounts, :uuid_col, false
    change_column_null :calendar_events, :uuid_col, false
    change_column_null :calendar_sync_logs, :uuid_col, false
    change_column_null :calendar_webhook_channels, :uuid_col, false
    change_column_null :calendars, :uuid_col, false
    change_column_null :connections, :uuid_col, false
    change_column_null :contact_email_aliases, :uuid_col, false
    change_column_null :contact_tags, :uuid_col, false
    change_column_null :contacts, :uuid_col, false
    change_column_null :devices, :uuid_col, false
    change_column_null :document_drive_uploads, :uuid_col, false
    change_column_null :document_email_messages, :uuid_col, false
    change_column_null :document_templates, :uuid_col, false
    change_column_null :document_types, :uuid_col, false
    change_column_null :documents, :uuid_col, false
    change_column_null :drive_folder_mappings, :uuid_col, false
    change_column_null :email_account_signatures, :uuid_col, false
    change_column_null :email_account_users, :uuid_col, false
    change_column_null :email_accounts, :uuid_col, false
    change_column_null :email_folders, :uuid_col, false
    change_column_null :email_message_tags, :uuid_col, false
    change_column_null :email_messages, :uuid_col, false
    change_column_null :email_scan_logs, :uuid_col, false
    change_column_null :email_threads, :uuid_col, false
    change_column_null :events, :uuid_col, false
    change_column_null :exports, :uuid_col, false
    change_column_null :feed_items, :uuid_col, false
    change_column_null :folder_memberships, :uuid_col, false
    change_column_null :google_drive_accounts, :uuid_col, false
    change_column_null :google_drive_configs, :uuid_col, false
    change_column_null :identities, :uuid_col, false
    change_column_null :invitations, :uuid_col, false
    change_column_null :mail_folders, :uuid_col, false
    change_column_null :mfa_email_challenges, :uuid_col, false
    change_column_null :notification_preferences, :uuid_col, false
    change_column_null :notifications, :uuid_col, false
    change_column_null :notion_database_mappings, :uuid_col, false
    change_column_null :notion_integrations, :uuid_col, false
    change_column_null :notion_pages, :uuid_col, false
    change_column_null :oauth_applications, :uuid_col, false
    change_column_null :organization_memberships, :uuid_col, false
    change_column_null :organizations, :uuid_col, false
    change_column_null :people, :uuid_col, false
    change_column_null :pipeline_memberships, :uuid_col, false
    change_column_null :pipeline_stages, :uuid_col, false
    change_column_null :pipelines, :uuid_col, false
    change_column_null :recovery_codes, :uuid_col, false
    change_column_null :reminders, :uuid_col, false
    change_column_null :scheduled_emails, :uuid_col, false
    change_column_null :search_chunks, :uuid_col, false
    change_column_null :search_records, :uuid_col, false
    change_column_null :search_tag_embeddings, :uuid_col, false
    change_column_null :sessions, :uuid_col, false
    change_column_null :signatures, :uuid_col, false
    change_column_null :signup_requests, :uuid_col, false
    change_column_null :skim_decisions, :uuid_col, false
    change_column_null :tags, :uuid_col, false
    change_column_null :templates, :uuid_col, false
    change_column_null :thread_follows, :uuid_col, false
    change_column_null :users, :uuid_col, false
    change_column_null :webauthn_credentials, :uuid_col, false
    change_column_null :workflow_execution_steps, :uuid_col, false
    change_column_null :workflow_executions, :uuid_col, false
    change_column_null :workflow_steps, :uuid_col, false
    change_column_null :workflows, :uuid_col, false
    change_column_null :workspaces, :uuid_col, false
    change_column_null :zoho_drive_accounts, :uuid_col, false
  end

  def down
    remove_column :zoho_drive_accounts, :uuid_col
    remove_column :workspaces, :uuid_col
    remove_column :workflows, :uuid_col
    remove_column :workflow_steps, :uuid_col
    remove_column :workflow_executions, :uuid_col
    remove_column :workflow_execution_steps, :uuid_col
    remove_column :webauthn_credentials, :uuid_col
    remove_column :users, :uuid_col
    remove_column :thread_follows, :uuid_col
    remove_column :templates, :uuid_col
    remove_column :tags, :uuid_col
    remove_column :skim_decisions, :uuid_col
    remove_column :signup_requests, :uuid_col
    remove_column :signatures, :uuid_col
    remove_column :sessions, :uuid_col
    remove_column :search_tag_embeddings, :uuid_col
    remove_column :search_records, :uuid_col
    remove_column :search_chunks, :uuid_col
    remove_column :scheduled_emails, :uuid_col
    remove_column :reminders, :uuid_col
    remove_column :recovery_codes, :uuid_col
    remove_column :pipelines, :uuid_col
    remove_column :pipeline_stages, :uuid_col
    remove_column :pipeline_memberships, :uuid_col
    remove_column :people, :uuid_col
    remove_column :organizations, :uuid_col
    remove_column :organization_memberships, :uuid_col
    remove_column :oauth_applications, :uuid_col
    remove_column :notion_pages, :uuid_col
    remove_column :notion_integrations, :uuid_col
    remove_column :notion_database_mappings, :uuid_col
    remove_column :notifications, :uuid_col
    remove_column :notification_preferences, :uuid_col
    remove_column :mfa_email_challenges, :uuid_col
    remove_column :mail_folders, :uuid_col
    remove_column :invitations, :uuid_col
    remove_column :identities, :uuid_col
    remove_column :google_drive_configs, :uuid_col
    remove_column :google_drive_accounts, :uuid_col
    remove_column :folder_memberships, :uuid_col
    remove_column :feed_items, :uuid_col
    remove_column :exports, :uuid_col
    remove_column :events, :uuid_col
    remove_column :email_threads, :uuid_col
    remove_column :email_scan_logs, :uuid_col
    remove_column :email_messages, :uuid_col
    remove_column :email_message_tags, :uuid_col
    remove_column :email_folders, :uuid_col
    remove_column :email_accounts, :uuid_col
    remove_column :email_account_users, :uuid_col
    remove_column :email_account_signatures, :uuid_col
    remove_column :drive_folder_mappings, :uuid_col
    remove_column :documents, :uuid_col
    remove_column :document_types, :uuid_col
    remove_column :document_templates, :uuid_col
    remove_column :document_email_messages, :uuid_col
    remove_column :document_drive_uploads, :uuid_col
    remove_column :devices, :uuid_col
    remove_column :contacts, :uuid_col
    remove_column :contact_tags, :uuid_col
    remove_column :contact_email_aliases, :uuid_col
    remove_column :connections, :uuid_col
    remove_column :calendars, :uuid_col
    remove_column :calendar_webhook_channels, :uuid_col
    remove_column :calendar_sync_logs, :uuid_col
    remove_column :calendar_events, :uuid_col
    remove_column :calendar_accounts, :uuid_col
    remove_column :calendar_account_users, :uuid_col
    remove_column :bug_reports, :uuid_col
    remove_column :beta_codes, :uuid_col
    remove_column :audit_events, :uuid_col
    remove_column :ai_configurations, :uuid_col
    remove_column :ai_adapters, :uuid_col
    remove_column :agent_threads, :uuid_col
    remove_column :agent_messages, :uuid_col
    remove_column :account_exports, :uuid_col
  end
end

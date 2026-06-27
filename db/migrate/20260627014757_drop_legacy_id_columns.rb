# frozen_string_literal: true

# Phase 3: Drop legacy_id columns.
class DropLegacyIdColumns < ActiveRecord::Migration[8.1]
  def up
    remove_column :account_exports, :legacy_id
    remove_column :agent_messages, :legacy_id
    remove_column :agent_threads, :legacy_id
    remove_column :ai_adapters, :legacy_id
    remove_column :ai_configurations, :legacy_id
    remove_column :audit_events, :legacy_id
    remove_column :beta_codes, :legacy_id
    remove_column :bug_reports, :legacy_id
    remove_column :calendar_account_users, :legacy_id
    remove_column :calendar_accounts, :legacy_id
    remove_column :calendar_events, :legacy_id
    remove_column :calendar_sync_logs, :legacy_id
    remove_column :calendar_webhook_channels, :legacy_id
    remove_column :calendars, :legacy_id
    remove_column :connections, :legacy_id
    remove_column :contact_email_aliases, :legacy_id
    remove_column :contact_tags, :legacy_id
    remove_column :contacts, :legacy_id
    remove_column :devices, :legacy_id
    remove_column :document_drive_uploads, :legacy_id
    remove_column :document_email_messages, :legacy_id
    remove_column :document_templates, :legacy_id
    remove_column :document_types, :legacy_id
    remove_column :documents, :legacy_id
    remove_column :drive_folder_mappings, :legacy_id
    remove_column :email_account_signatures, :legacy_id
    remove_column :email_account_users, :legacy_id
    remove_column :email_accounts, :legacy_id
    remove_column :email_folders, :legacy_id
    remove_column :email_message_tags, :legacy_id
    remove_column :email_messages, :legacy_id
    remove_column :email_scan_logs, :legacy_id
    remove_column :email_threads, :legacy_id
    remove_column :events, :legacy_id
    remove_column :exports, :legacy_id
    remove_column :feed_items, :legacy_id
    remove_column :folder_memberships, :legacy_id
    remove_column :google_drive_accounts, :legacy_id
    remove_column :google_drive_configs, :legacy_id
    remove_column :identities, :legacy_id
    remove_column :invitations, :legacy_id
    remove_column :mail_folders, :legacy_id
    remove_column :mfa_email_challenges, :legacy_id
    remove_column :notification_preferences, :legacy_id
    remove_column :notifications, :legacy_id
    remove_column :notion_database_mappings, :legacy_id
    remove_column :notion_integrations, :legacy_id
    remove_column :notion_pages, :legacy_id
    remove_column :oauth_applications, :legacy_id
    remove_column :organization_memberships, :legacy_id
    remove_column :organizations, :legacy_id
    remove_column :people, :legacy_id
    remove_column :pipeline_memberships, :legacy_id
    remove_column :pipeline_stages, :legacy_id
    remove_column :pipelines, :legacy_id
    remove_column :recovery_codes, :legacy_id
    remove_column :reminders, :legacy_id
    remove_column :scheduled_emails, :legacy_id
    remove_column :search_chunks, :legacy_id
    remove_column :search_records, :legacy_id
    remove_column :search_tag_embeddings, :legacy_id
    remove_column :sessions, :legacy_id
    remove_column :signatures, :legacy_id
    remove_column :signup_requests, :legacy_id
    remove_column :skim_decisions, :legacy_id
    remove_column :tags, :legacy_id
    remove_column :templates, :legacy_id
    remove_column :thread_follows, :legacy_id
    remove_column :users, :legacy_id
    remove_column :webauthn_credentials, :legacy_id
    remove_column :workflow_execution_steps, :legacy_id
    remove_column :workflow_executions, :legacy_id
    remove_column :workflow_steps, :legacy_id
    remove_column :workflows, :legacy_id
    remove_column :workspaces, :legacy_id
    remove_column :zoho_drive_accounts, :legacy_id
  end
  def down
    raise ActiveRecord::IrreversibleMigration
  end
end

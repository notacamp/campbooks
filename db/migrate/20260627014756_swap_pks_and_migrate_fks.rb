# frozen_string_literal: true

# Phase 2: For each domain table (root → leaf):
#   a) Drop incoming FK constraints (including from framework tables)
#   b) Swap PK (id → legacy_id, uuid_col → id)
#   c) Migrate FK columns on referencing tables
#   d) Recreate FK constraints
class SwapPksAndMigrateFks < ActiveRecord::Migration[8.1]
  def up
    # ===== workflow_execution_steps =====
    # Swap PK
    execute "ALTER TABLE workflow_execution_steps DROP CONSTRAINT workflow_execution_steps_pkey"
    rename_column :workflow_execution_steps, :id, :legacy_id
    rename_column :workflow_execution_steps, :uuid_col, :id
    execute "ALTER TABLE workflow_execution_steps ADD PRIMARY KEY (id)"

    # ===== workflow_steps =====
    # Drop incoming FK constraints
    remove_foreign_key :workflow_execution_steps, :workflow_steps, column: :workflow_step_id rescue nil
    # Swap PK
    execute "ALTER TABLE workflow_steps DROP CONSTRAINT workflow_steps_pkey"
    rename_column :workflow_steps, :id, :legacy_id
    rename_column :workflow_steps, :uuid_col, :id
    execute "ALTER TABLE workflow_steps ADD PRIMARY KEY (id)"
    # Migrate FK columns on referencing tables
    add_column :workflow_execution_steps, :new_workflow_step_id, :uuid
    execute <<~SQL
      UPDATE workflow_execution_steps
      SET new_workflow_step_id = workflow_steps.id
      FROM workflow_steps
      WHERE workflow_execution_steps.workflow_step_id = workflow_steps.legacy_id
    SQL
    remove_column :workflow_execution_steps, :workflow_step_id
    rename_column :workflow_execution_steps, :new_workflow_step_id, :workflow_step_id
    # Recreate FK constraints
    add_foreign_key :workflow_execution_steps, :workflow_steps, column: :workflow_step_id

    # ===== workflow_executions =====
    # Drop incoming FK constraints
    remove_foreign_key :workflow_execution_steps, :workflow_executions, column: :workflow_execution_id rescue nil
    # Swap PK
    execute "ALTER TABLE workflow_executions DROP CONSTRAINT workflow_executions_pkey"
    rename_column :workflow_executions, :id, :legacy_id
    rename_column :workflow_executions, :uuid_col, :id
    execute "ALTER TABLE workflow_executions ADD PRIMARY KEY (id)"
    # Migrate FK columns on referencing tables
    add_column :workflow_execution_steps, :new_workflow_execution_id, :uuid
    execute <<~SQL
      UPDATE workflow_execution_steps
      SET new_workflow_execution_id = workflow_executions.id
      FROM workflow_executions
      WHERE workflow_execution_steps.workflow_execution_id = workflow_executions.legacy_id
    SQL
    remove_column :workflow_execution_steps, :workflow_execution_id
    rename_column :workflow_execution_steps, :new_workflow_execution_id, :workflow_execution_id
    # Recreate FK constraints
    add_foreign_key :workflow_execution_steps, :workflow_executions, column: :workflow_execution_id

    # ===== workflows =====
    # Drop incoming FK constraints
    remove_foreign_key :workflow_executions, :workflows, column: :workflow_id rescue nil
    remove_foreign_key :workflow_steps, :workflows, column: :workflow_id rescue nil
    # Swap PK
    execute "ALTER TABLE workflows DROP CONSTRAINT workflows_pkey"
    rename_column :workflows, :id, :legacy_id
    rename_column :workflows, :uuid_col, :id
    execute "ALTER TABLE workflows ADD PRIMARY KEY (id)"
    # Migrate FK columns on referencing tables
    add_column :workflow_executions, :new_workflow_id, :uuid
    execute <<~SQL
      UPDATE workflow_executions
      SET new_workflow_id = workflows.id
      FROM workflows
      WHERE workflow_executions.workflow_id = workflows.legacy_id
    SQL
    remove_column :workflow_executions, :workflow_id
    rename_column :workflow_executions, :new_workflow_id, :workflow_id
    add_column :workflow_steps, :new_workflow_id, :uuid
    execute <<~SQL
      UPDATE workflow_steps
      SET new_workflow_id = workflows.id
      FROM workflows
      WHERE workflow_steps.workflow_id = workflows.legacy_id
    SQL
    remove_column :workflow_steps, :workflow_id
    rename_column :workflow_steps, :new_workflow_id, :workflow_id
    # Recreate FK constraints
    add_foreign_key :workflow_executions, :workflows, column: :workflow_id
    add_foreign_key :workflow_steps, :workflows, column: :workflow_id

    # ===== webauthn_credentials =====
    # Swap PK
    execute "ALTER TABLE webauthn_credentials DROP CONSTRAINT webauthn_credentials_pkey"
    rename_column :webauthn_credentials, :id, :legacy_id
    rename_column :webauthn_credentials, :uuid_col, :id
    execute "ALTER TABLE webauthn_credentials ADD PRIMARY KEY (id)"

    # ===== thread_follows =====
    # Swap PK
    execute "ALTER TABLE thread_follows DROP CONSTRAINT thread_follows_pkey"
    rename_column :thread_follows, :id, :legacy_id
    rename_column :thread_follows, :uuid_col, :id
    execute "ALTER TABLE thread_follows ADD PRIMARY KEY (id)"

    # ===== templates =====
    # Swap PK
    execute "ALTER TABLE templates DROP CONSTRAINT templates_pkey"
    rename_column :templates, :id, :legacy_id
    rename_column :templates, :uuid_col, :id
    execute "ALTER TABLE templates ADD PRIMARY KEY (id)"

    # ===== skim_decisions =====
    # Swap PK
    execute "ALTER TABLE skim_decisions DROP CONSTRAINT skim_decisions_pkey"
    rename_column :skim_decisions, :id, :legacy_id
    rename_column :skim_decisions, :uuid_col, :id
    execute "ALTER TABLE skim_decisions ADD PRIMARY KEY (id)"

    # ===== signup_requests =====
    # Swap PK
    execute "ALTER TABLE signup_requests DROP CONSTRAINT signup_requests_pkey"
    rename_column :signup_requests, :id, :legacy_id
    rename_column :signup_requests, :uuid_col, :id
    execute "ALTER TABLE signup_requests ADD PRIMARY KEY (id)"

    # ===== sessions =====
    # Swap PK
    execute "ALTER TABLE sessions DROP CONSTRAINT sessions_pkey"
    rename_column :sessions, :id, :legacy_id
    rename_column :sessions, :uuid_col, :id
    execute "ALTER TABLE sessions ADD PRIMARY KEY (id)"

    # ===== search_tag_embeddings =====
    # Swap PK
    execute "ALTER TABLE search_tag_embeddings DROP CONSTRAINT search_tag_embeddings_pkey"
    rename_column :search_tag_embeddings, :id, :legacy_id
    rename_column :search_tag_embeddings, :uuid_col, :id
    execute "ALTER TABLE search_tag_embeddings ADD PRIMARY KEY (id)"

    # ===== search_records =====
    # Swap PK
    execute "ALTER TABLE search_records DROP CONSTRAINT search_records_pkey"
    rename_column :search_records, :id, :legacy_id
    rename_column :search_records, :uuid_col, :id
    execute "ALTER TABLE search_records ADD PRIMARY KEY (id)"

    # ===== search_chunks =====
    # Swap PK
    execute "ALTER TABLE search_chunks DROP CONSTRAINT search_chunks_pkey"
    rename_column :search_chunks, :id, :legacy_id
    rename_column :search_chunks, :uuid_col, :id
    execute "ALTER TABLE search_chunks ADD PRIMARY KEY (id)"

    # ===== scheduled_emails =====
    # Swap PK
    execute "ALTER TABLE scheduled_emails DROP CONSTRAINT scheduled_emails_pkey"
    rename_column :scheduled_emails, :id, :legacy_id
    rename_column :scheduled_emails, :uuid_col, :id
    execute "ALTER TABLE scheduled_emails ADD PRIMARY KEY (id)"

    # ===== reminders =====
    # Swap PK
    execute "ALTER TABLE reminders DROP CONSTRAINT reminders_pkey"
    rename_column :reminders, :id, :legacy_id
    rename_column :reminders, :uuid_col, :id
    execute "ALTER TABLE reminders ADD PRIMARY KEY (id)"

    # ===== recovery_codes =====
    # Swap PK
    execute "ALTER TABLE recovery_codes DROP CONSTRAINT recovery_codes_pkey"
    rename_column :recovery_codes, :id, :legacy_id
    rename_column :recovery_codes, :uuid_col, :id
    execute "ALTER TABLE recovery_codes ADD PRIMARY KEY (id)"

    # ===== pipeline_memberships =====
    # Swap PK
    execute "ALTER TABLE pipeline_memberships DROP CONSTRAINT pipeline_memberships_pkey"
    rename_column :pipeline_memberships, :id, :legacy_id
    rename_column :pipeline_memberships, :uuid_col, :id
    execute "ALTER TABLE pipeline_memberships ADD PRIMARY KEY (id)"

    # ===== pipeline_stages =====
    # Drop incoming FK constraints
    remove_foreign_key :pipeline_memberships, :pipeline_stages, column: :current_stage_id rescue nil
    # Swap PK
    execute "ALTER TABLE pipeline_stages DROP CONSTRAINT pipeline_stages_pkey"
    rename_column :pipeline_stages, :id, :legacy_id
    rename_column :pipeline_stages, :uuid_col, :id
    execute "ALTER TABLE pipeline_stages ADD PRIMARY KEY (id)"
    # Migrate FK columns on referencing tables
    add_column :pipeline_memberships, :new_current_stage_id, :uuid
    execute <<~SQL
      UPDATE pipeline_memberships
      SET new_current_stage_id = pipeline_stages.id
      FROM pipeline_stages
      WHERE pipeline_memberships.current_stage_id = pipeline_stages.legacy_id
    SQL
    remove_column :pipeline_memberships, :current_stage_id
    rename_column :pipeline_memberships, :new_current_stage_id, :current_stage_id
    # Recreate FK constraints
    add_foreign_key :pipeline_memberships, :pipeline_stages, column: :current_stage_id

    # ===== pipelines =====
    # Drop incoming FK constraints
    remove_foreign_key :pipeline_memberships, :pipelines, column: :pipeline_id rescue nil
    remove_foreign_key :pipeline_stages, :pipelines, column: :pipeline_id rescue nil
    # Swap PK
    execute "ALTER TABLE pipelines DROP CONSTRAINT pipelines_pkey"
    rename_column :pipelines, :id, :legacy_id
    rename_column :pipelines, :uuid_col, :id
    execute "ALTER TABLE pipelines ADD PRIMARY KEY (id)"
    # Migrate FK columns on referencing tables
    add_column :pipeline_memberships, :new_pipeline_id, :uuid
    execute <<~SQL
      UPDATE pipeline_memberships
      SET new_pipeline_id = pipelines.id
      FROM pipelines
      WHERE pipeline_memberships.pipeline_id = pipelines.legacy_id
    SQL
    remove_column :pipeline_memberships, :pipeline_id
    rename_column :pipeline_memberships, :new_pipeline_id, :pipeline_id
    add_column :pipeline_stages, :new_pipeline_id, :uuid
    execute <<~SQL
      UPDATE pipeline_stages
      SET new_pipeline_id = pipelines.id
      FROM pipelines
      WHERE pipeline_stages.pipeline_id = pipelines.legacy_id
    SQL
    remove_column :pipeline_stages, :pipeline_id
    rename_column :pipeline_stages, :new_pipeline_id, :pipeline_id
    # Recreate FK constraints
    add_foreign_key :pipeline_memberships, :pipelines, column: :pipeline_id
    add_foreign_key :pipeline_stages, :pipelines, column: :pipeline_id

    # ===== organization_memberships =====
    # Swap PK
    execute "ALTER TABLE organization_memberships DROP CONSTRAINT organization_memberships_pkey"
    rename_column :organization_memberships, :id, :legacy_id
    rename_column :organization_memberships, :uuid_col, :id
    execute "ALTER TABLE organization_memberships ADD PRIMARY KEY (id)"

    # ===== organizations =====
    # Drop incoming FK constraints
    remove_foreign_key :organization_memberships, :organizations, column: :organization_id rescue nil
    # Swap PK
    execute "ALTER TABLE organizations DROP CONSTRAINT organizations_pkey"
    rename_column :organizations, :id, :legacy_id
    rename_column :organizations, :uuid_col, :id
    execute "ALTER TABLE organizations ADD PRIMARY KEY (id)"
    # Migrate FK columns on referencing tables
    add_column :organization_memberships, :new_organization_id, :uuid
    execute <<~SQL
      UPDATE organization_memberships
      SET new_organization_id = organizations.id
      FROM organizations
      WHERE organization_memberships.organization_id = organizations.legacy_id
    SQL
    remove_column :organization_memberships, :organization_id
    rename_column :organization_memberships, :new_organization_id, :organization_id
    # Recreate FK constraints
    add_foreign_key :organization_memberships, :organizations, column: :organization_id

    # ===== oauth_applications =====
    # Swap PK
    execute "ALTER TABLE oauth_access_grants DROP CONSTRAINT IF EXISTS fk_rails_b4b53e07b8"
    execute "ALTER TABLE oauth_access_tokens DROP CONSTRAINT IF EXISTS fk_rails_732cb83ab7"
    execute "ALTER TABLE oauth_applications DROP CONSTRAINT oauth_applications_pkey"
    rename_column :oauth_applications, :id, :legacy_id
    rename_column :oauth_applications, :uuid_col, :id
    execute "ALTER TABLE oauth_applications ADD PRIMARY KEY (id)"
    %w[oauth_access_grants oauth_access_tokens].each do |tbl|
      add_column tbl, :new_application_id, :uuid
      execute "UPDATE #{tbl} SET new_application_id = oauth_applications.id FROM oauth_applications WHERE #{tbl}.application_id = oauth_applications.legacy_id"
      remove_column tbl, :application_id
      rename_column tbl, :new_application_id, :application_id
    end
    add_foreign_key :oauth_access_grants, :oauth_applications, column: :application_id
    add_foreign_key :oauth_access_tokens, :oauth_applications, column: :application_id

    # ===== notion_pages =====
    # Swap PK
    execute "ALTER TABLE notion_pages DROP CONSTRAINT notion_pages_pkey"
    rename_column :notion_pages, :id, :legacy_id
    rename_column :notion_pages, :uuid_col, :id
    execute "ALTER TABLE notion_pages ADD PRIMARY KEY (id)"

    # ===== notion_integrations =====
    # Swap PK
    execute "ALTER TABLE notion_integrations DROP CONSTRAINT notion_integrations_pkey"
    rename_column :notion_integrations, :id, :legacy_id
    rename_column :notion_integrations, :uuid_col, :id
    execute "ALTER TABLE notion_integrations ADD PRIMARY KEY (id)"

    # ===== notion_database_mappings =====
    # Drop incoming FK constraints
    remove_foreign_key :notion_pages, :notion_database_mappings, column: :notion_database_mapping_id rescue nil
    # Swap PK
    execute "ALTER TABLE notion_database_mappings DROP CONSTRAINT notion_database_mappings_pkey"
    rename_column :notion_database_mappings, :id, :legacy_id
    rename_column :notion_database_mappings, :uuid_col, :id
    execute "ALTER TABLE notion_database_mappings ADD PRIMARY KEY (id)"
    # Migrate FK columns on referencing tables
    add_column :notion_pages, :new_notion_database_mapping_id, :uuid
    execute <<~SQL
      UPDATE notion_pages
      SET new_notion_database_mapping_id = notion_database_mappings.id
      FROM notion_database_mappings
      WHERE notion_pages.notion_database_mapping_id = notion_database_mappings.legacy_id
    SQL
    remove_column :notion_pages, :notion_database_mapping_id
    rename_column :notion_pages, :new_notion_database_mapping_id, :notion_database_mapping_id
    # Recreate FK constraints
    add_foreign_key :notion_pages, :notion_database_mappings, column: :notion_database_mapping_id

    # ===== notifications =====
    # Swap PK
    execute "ALTER TABLE notifications DROP CONSTRAINT notifications_pkey"
    rename_column :notifications, :id, :legacy_id
    rename_column :notifications, :uuid_col, :id
    execute "ALTER TABLE notifications ADD PRIMARY KEY (id)"

    # ===== notification_preferences =====
    # Swap PK
    execute "ALTER TABLE notification_preferences DROP CONSTRAINT notification_preferences_pkey"
    rename_column :notification_preferences, :id, :legacy_id
    rename_column :notification_preferences, :uuid_col, :id
    execute "ALTER TABLE notification_preferences ADD PRIMARY KEY (id)"

    # ===== mfa_email_challenges =====
    # Swap PK
    execute "ALTER TABLE mfa_email_challenges DROP CONSTRAINT mfa_email_challenges_pkey"
    rename_column :mfa_email_challenges, :id, :legacy_id
    rename_column :mfa_email_challenges, :uuid_col, :id
    execute "ALTER TABLE mfa_email_challenges ADD PRIMARY KEY (id)"

    # ===== invitations =====
    # Swap PK
    execute "ALTER TABLE invitations DROP CONSTRAINT invitations_pkey"
    rename_column :invitations, :id, :legacy_id
    rename_column :invitations, :uuid_col, :id
    execute "ALTER TABLE invitations ADD PRIMARY KEY (id)"

    # ===== identities =====
    # Swap PK
    execute "ALTER TABLE identities DROP CONSTRAINT identities_pkey"
    rename_column :identities, :id, :legacy_id
    rename_column :identities, :uuid_col, :id
    execute "ALTER TABLE identities ADD PRIMARY KEY (id)"

    # ===== google_drive_configs =====
    # Swap PK
    execute "ALTER TABLE google_drive_configs DROP CONSTRAINT google_drive_configs_pkey"
    rename_column :google_drive_configs, :id, :legacy_id
    rename_column :google_drive_configs, :uuid_col, :id
    execute "ALTER TABLE google_drive_configs ADD PRIMARY KEY (id)"

    # ===== google_drive_accounts =====
    # Swap PK
    execute "ALTER TABLE google_drive_accounts DROP CONSTRAINT google_drive_accounts_pkey"
    rename_column :google_drive_accounts, :id, :legacy_id
    rename_column :google_drive_accounts, :uuid_col, :id
    execute "ALTER TABLE google_drive_accounts ADD PRIMARY KEY (id)"

    # ===== folder_memberships =====
    # Swap PK
    execute "ALTER TABLE folder_memberships DROP CONSTRAINT folder_memberships_pkey"
    rename_column :folder_memberships, :id, :legacy_id
    rename_column :folder_memberships, :uuid_col, :id
    execute "ALTER TABLE folder_memberships ADD PRIMARY KEY (id)"

    # ===== mail_folders =====
    # Drop incoming FK constraints
    remove_foreign_key :folder_memberships, :mail_folders, column: :mail_folder_id rescue nil
    remove_foreign_key :mail_folders, :mail_folders, column: :parent_id rescue nil
    # Swap PK
    execute "ALTER TABLE mail_folders DROP CONSTRAINT mail_folders_pkey"
    rename_column :mail_folders, :id, :legacy_id
    rename_column :mail_folders, :uuid_col, :id
    execute "ALTER TABLE mail_folders ADD PRIMARY KEY (id)"
    # Migrate FK columns on referencing tables
    add_column :folder_memberships, :new_mail_folder_id, :uuid
    execute <<~SQL
      UPDATE folder_memberships
      SET new_mail_folder_id = mail_folders.id
      FROM mail_folders
      WHERE folder_memberships.mail_folder_id = mail_folders.legacy_id
    SQL
    remove_column :folder_memberships, :mail_folder_id
    rename_column :folder_memberships, :new_mail_folder_id, :mail_folder_id
    add_column :mail_folders, :new_parent_id, :uuid
    execute <<~SQL
      UPDATE mail_folders
      SET new_parent_id = parent_tbl.id
      FROM mail_folders AS parent_tbl
      WHERE mail_folders.parent_id = parent_tbl.legacy_id
    SQL
    remove_column :mail_folders, :parent_id
    rename_column :mail_folders, :new_parent_id, :parent_id
    # Recreate FK constraints
    add_foreign_key :folder_memberships, :mail_folders, column: :mail_folder_id
    add_foreign_key :mail_folders, :mail_folders, column: :parent_id

    # ===== feed_items =====
    # Swap PK
    execute "ALTER TABLE feed_items DROP CONSTRAINT feed_items_pkey"
    rename_column :feed_items, :id, :legacy_id
    rename_column :feed_items, :uuid_col, :id
    execute "ALTER TABLE feed_items ADD PRIMARY KEY (id)"

    # ===== exports =====
    # Swap PK
    execute "ALTER TABLE exports DROP CONSTRAINT exports_pkey"
    rename_column :exports, :id, :legacy_id
    rename_column :exports, :uuid_col, :id
    execute "ALTER TABLE exports ADD PRIMARY KEY (id)"

    # ===== events =====
    # Drop incoming FK constraints
    remove_foreign_key :events, :events, column: :caused_by_event_id rescue nil
    # Swap PK
    execute "ALTER TABLE events DROP CONSTRAINT events_pkey"
    rename_column :events, :id, :legacy_id
    rename_column :events, :uuid_col, :id
    execute "ALTER TABLE events ADD PRIMARY KEY (id)"
    # Migrate FK columns on referencing tables
    add_column :events, :new_caused_by_event_id, :uuid
    execute <<~SQL
      UPDATE events
      SET new_caused_by_event_id = parent_tbl.id
      FROM events AS parent_tbl
      WHERE events.caused_by_event_id = parent_tbl.legacy_id
    SQL
    remove_column :events, :caused_by_event_id
    rename_column :events, :new_caused_by_event_id, :caused_by_event_id
    # Recreate FK constraints
    add_foreign_key :events, :events, column: :caused_by_event_id

    # ===== email_message_tags =====
    # Swap PK
    execute "ALTER TABLE email_message_tags DROP CONSTRAINT email_message_tags_pkey"
    rename_column :email_message_tags, :id, :legacy_id
    rename_column :email_message_tags, :uuid_col, :id
    execute "ALTER TABLE email_message_tags ADD PRIMARY KEY (id)"

    # ===== email_folders =====
    # Swap PK
    execute "ALTER TABLE email_folders DROP CONSTRAINT email_folders_pkey"
    rename_column :email_folders, :id, :legacy_id
    rename_column :email_folders, :uuid_col, :id
    execute "ALTER TABLE email_folders ADD PRIMARY KEY (id)"

    # ===== email_account_users =====
    # Swap PK
    execute "ALTER TABLE email_account_users DROP CONSTRAINT email_account_users_pkey"
    rename_column :email_account_users, :id, :legacy_id
    rename_column :email_account_users, :uuid_col, :id
    execute "ALTER TABLE email_account_users ADD PRIMARY KEY (id)"

    # ===== email_account_signatures =====
    # Swap PK
    execute "ALTER TABLE email_account_signatures DROP CONSTRAINT email_account_signatures_pkey"
    rename_column :email_account_signatures, :id, :legacy_id
    rename_column :email_account_signatures, :uuid_col, :id
    execute "ALTER TABLE email_account_signatures ADD PRIMARY KEY (id)"

    # ===== signatures =====
    # Drop incoming FK constraints
    remove_foreign_key :email_account_signatures, :signatures, column: :signature_id rescue nil
    # Swap PK
    execute "ALTER TABLE signatures DROP CONSTRAINT signatures_pkey"
    rename_column :signatures, :id, :legacy_id
    rename_column :signatures, :uuid_col, :id
    execute "ALTER TABLE signatures ADD PRIMARY KEY (id)"
    # Migrate FK columns on referencing tables
    add_column :email_account_signatures, :new_signature_id, :uuid
    execute <<~SQL
      UPDATE email_account_signatures
      SET new_signature_id = signatures.id
      FROM signatures
      WHERE email_account_signatures.signature_id = signatures.legacy_id
    SQL
    remove_column :email_account_signatures, :signature_id
    rename_column :email_account_signatures, :new_signature_id, :signature_id
    # Recreate FK constraints
    add_foreign_key :email_account_signatures, :signatures, column: :signature_id

    # ===== drive_folder_mappings =====
    # Swap PK
    execute "ALTER TABLE drive_folder_mappings DROP CONSTRAINT drive_folder_mappings_pkey"
    rename_column :drive_folder_mappings, :id, :legacy_id
    rename_column :drive_folder_mappings, :uuid_col, :id
    execute "ALTER TABLE drive_folder_mappings ADD PRIMARY KEY (id)"

    # ===== document_templates =====
    # Swap PK
    execute "ALTER TABLE document_templates DROP CONSTRAINT document_templates_pkey"
    rename_column :document_templates, :id, :legacy_id
    rename_column :document_templates, :uuid_col, :id
    execute "ALTER TABLE document_templates ADD PRIMARY KEY (id)"

    # ===== document_email_messages =====
    # Swap PK
    execute "ALTER TABLE document_email_messages DROP CONSTRAINT document_email_messages_pkey"
    rename_column :document_email_messages, :id, :legacy_id
    rename_column :document_email_messages, :uuid_col, :id
    execute "ALTER TABLE document_email_messages ADD PRIMARY KEY (id)"

    # ===== document_drive_uploads =====
    # Swap PK
    execute "ALTER TABLE document_drive_uploads DROP CONSTRAINT document_drive_uploads_pkey"
    rename_column :document_drive_uploads, :id, :legacy_id
    rename_column :document_drive_uploads, :uuid_col, :id
    execute "ALTER TABLE document_drive_uploads ADD PRIMARY KEY (id)"

    # ===== zoho_drive_accounts =====
    # Drop incoming FK constraints
    remove_foreign_key :document_drive_uploads, :zoho_drive_accounts, column: :zoho_drive_account_id rescue nil
    remove_foreign_key :drive_folder_mappings, :zoho_drive_accounts, column: :zoho_drive_account_id rescue nil
    # Swap PK
    execute "ALTER TABLE zoho_drive_accounts DROP CONSTRAINT zoho_drive_accounts_pkey"
    rename_column :zoho_drive_accounts, :id, :legacy_id
    rename_column :zoho_drive_accounts, :uuid_col, :id
    execute "ALTER TABLE zoho_drive_accounts ADD PRIMARY KEY (id)"
    # Migrate FK columns on referencing tables
    add_column :document_drive_uploads, :new_zoho_drive_account_id, :uuid
    execute <<~SQL
      UPDATE document_drive_uploads
      SET new_zoho_drive_account_id = zoho_drive_accounts.id
      FROM zoho_drive_accounts
      WHERE document_drive_uploads.zoho_drive_account_id = zoho_drive_accounts.legacy_id
    SQL
    remove_column :document_drive_uploads, :zoho_drive_account_id
    rename_column :document_drive_uploads, :new_zoho_drive_account_id, :zoho_drive_account_id
    add_column :drive_folder_mappings, :new_zoho_drive_account_id, :uuid
    execute <<~SQL
      UPDATE drive_folder_mappings
      SET new_zoho_drive_account_id = zoho_drive_accounts.id
      FROM zoho_drive_accounts
      WHERE drive_folder_mappings.zoho_drive_account_id = zoho_drive_accounts.legacy_id
    SQL
    remove_column :drive_folder_mappings, :zoho_drive_account_id
    rename_column :drive_folder_mappings, :new_zoho_drive_account_id, :zoho_drive_account_id
    # Recreate FK constraints
    add_foreign_key :document_drive_uploads, :zoho_drive_accounts, column: :zoho_drive_account_id
    add_foreign_key :drive_folder_mappings, :zoho_drive_accounts, column: :zoho_drive_account_id

    # ===== documents =====
    # Drop incoming FK constraints
    remove_foreign_key :document_drive_uploads, :documents, column: :document_id rescue nil
    remove_foreign_key :document_email_messages, :documents, column: :document_id rescue nil
    remove_foreign_key :notion_pages, :documents, column: :document_id rescue nil
    # Swap PK
    execute "ALTER TABLE documents DROP CONSTRAINT documents_pkey"
    rename_column :documents, :id, :legacy_id
    rename_column :documents, :uuid_col, :id
    execute "ALTER TABLE documents ADD PRIMARY KEY (id)"
    # Migrate FK columns on referencing tables
    add_column :document_drive_uploads, :new_document_id, :uuid
    execute <<~SQL
      UPDATE document_drive_uploads
      SET new_document_id = documents.id
      FROM documents
      WHERE document_drive_uploads.document_id = documents.legacy_id
    SQL
    remove_column :document_drive_uploads, :document_id
    rename_column :document_drive_uploads, :new_document_id, :document_id
    add_column :document_email_messages, :new_document_id, :uuid
    execute <<~SQL
      UPDATE document_email_messages
      SET new_document_id = documents.id
      FROM documents
      WHERE document_email_messages.document_id = documents.legacy_id
    SQL
    remove_column :document_email_messages, :document_id
    rename_column :document_email_messages, :new_document_id, :document_id
    add_column :notion_pages, :new_document_id, :uuid
    execute <<~SQL
      UPDATE notion_pages
      SET new_document_id = documents.id
      FROM documents
      WHERE notion_pages.document_id = documents.legacy_id
    SQL
    remove_column :notion_pages, :document_id
    rename_column :notion_pages, :new_document_id, :document_id
    # Recreate FK constraints
    add_foreign_key :document_drive_uploads, :documents, column: :document_id
    add_foreign_key :document_email_messages, :documents, column: :document_id
    add_foreign_key :notion_pages, :documents, column: :document_id

    # ===== document_types =====
    # Drop incoming FK constraints
    remove_foreign_key :documents, :document_types, column: :document_type_id rescue nil
    remove_foreign_key :drive_folder_mappings, :document_types, column: :document_type_id rescue nil
    remove_foreign_key :google_drive_configs, :document_types, column: :document_type_id rescue nil
    remove_foreign_key :notification_preferences, :document_types, column: :document_type_id rescue nil
    remove_foreign_key :notion_database_mappings, :document_types, column: :document_type_id rescue nil
    # Swap PK
    execute "ALTER TABLE document_types DROP CONSTRAINT document_types_pkey"
    rename_column :document_types, :id, :legacy_id
    rename_column :document_types, :uuid_col, :id
    execute "ALTER TABLE document_types ADD PRIMARY KEY (id)"
    # Migrate FK columns on referencing tables
    add_column :documents, :new_document_type_id, :uuid
    execute <<~SQL
      UPDATE documents
      SET new_document_type_id = document_types.id
      FROM document_types
      WHERE documents.document_type_id = document_types.legacy_id
    SQL
    remove_column :documents, :document_type_id
    rename_column :documents, :new_document_type_id, :document_type_id
    add_column :drive_folder_mappings, :new_document_type_id, :uuid
    execute <<~SQL
      UPDATE drive_folder_mappings
      SET new_document_type_id = document_types.id
      FROM document_types
      WHERE drive_folder_mappings.document_type_id = document_types.legacy_id
    SQL
    remove_column :drive_folder_mappings, :document_type_id
    rename_column :drive_folder_mappings, :new_document_type_id, :document_type_id
    add_column :google_drive_configs, :new_document_type_id, :uuid
    execute <<~SQL
      UPDATE google_drive_configs
      SET new_document_type_id = document_types.id
      FROM document_types
      WHERE google_drive_configs.document_type_id = document_types.legacy_id
    SQL
    remove_column :google_drive_configs, :document_type_id
    rename_column :google_drive_configs, :new_document_type_id, :document_type_id
    add_column :notification_preferences, :new_document_type_id, :uuid
    execute <<~SQL
      UPDATE notification_preferences
      SET new_document_type_id = document_types.id
      FROM document_types
      WHERE notification_preferences.document_type_id = document_types.legacy_id
    SQL
    remove_column :notification_preferences, :document_type_id
    rename_column :notification_preferences, :new_document_type_id, :document_type_id
    add_column :notion_database_mappings, :new_document_type_id, :uuid
    execute <<~SQL
      UPDATE notion_database_mappings
      SET new_document_type_id = document_types.id
      FROM document_types
      WHERE notion_database_mappings.document_type_id = document_types.legacy_id
    SQL
    remove_column :notion_database_mappings, :document_type_id
    rename_column :notion_database_mappings, :new_document_type_id, :document_type_id
    # Recreate FK constraints
    add_foreign_key :documents, :document_types, column: :document_type_id
    add_foreign_key :drive_folder_mappings, :document_types, column: :document_type_id
    add_foreign_key :google_drive_configs, :document_types, column: :document_type_id
    add_foreign_key :notification_preferences, :document_types, column: :document_type_id
    add_foreign_key :notion_database_mappings, :document_types, column: :document_type_id

    # ===== devices =====
    # Swap PK
    execute "ALTER TABLE devices DROP CONSTRAINT devices_pkey"
    rename_column :devices, :id, :legacy_id
    rename_column :devices, :uuid_col, :id
    execute "ALTER TABLE devices ADD PRIMARY KEY (id)"

    # ===== contact_tags =====
    # Swap PK
    execute "ALTER TABLE contact_tags DROP CONSTRAINT contact_tags_pkey"
    rename_column :contact_tags, :id, :legacy_id
    rename_column :contact_tags, :uuid_col, :id
    execute "ALTER TABLE contact_tags ADD PRIMARY KEY (id)"

    # ===== tags =====
    # Drop incoming FK constraints
    remove_foreign_key :contact_tags, :tags, column: :tag_id rescue nil
    remove_foreign_key :email_message_tags, :tags, column: :tag_id rescue nil
    remove_foreign_key :notification_preferences, :tags, column: :tag_id rescue nil
    remove_foreign_key :search_tag_embeddings, :tags, column: :tag_id rescue nil
    # Swap PK
    execute "ALTER TABLE tags DROP CONSTRAINT tags_pkey"
    rename_column :tags, :id, :legacy_id
    rename_column :tags, :uuid_col, :id
    execute "ALTER TABLE tags ADD PRIMARY KEY (id)"
    # Migrate FK columns on referencing tables
    add_column :contact_tags, :new_tag_id, :uuid
    execute <<~SQL
      UPDATE contact_tags
      SET new_tag_id = tags.id
      FROM tags
      WHERE contact_tags.tag_id = tags.legacy_id
    SQL
    remove_column :contact_tags, :tag_id
    rename_column :contact_tags, :new_tag_id, :tag_id
    add_column :email_message_tags, :new_tag_id, :uuid
    execute <<~SQL
      UPDATE email_message_tags
      SET new_tag_id = tags.id
      FROM tags
      WHERE email_message_tags.tag_id = tags.legacy_id
    SQL
    remove_column :email_message_tags, :tag_id
    rename_column :email_message_tags, :new_tag_id, :tag_id
    add_column :notification_preferences, :new_tag_id, :uuid
    execute <<~SQL
      UPDATE notification_preferences
      SET new_tag_id = tags.id
      FROM tags
      WHERE notification_preferences.tag_id = tags.legacy_id
    SQL
    remove_column :notification_preferences, :tag_id
    rename_column :notification_preferences, :new_tag_id, :tag_id
    add_column :search_tag_embeddings, :new_tag_id, :uuid
    execute <<~SQL
      UPDATE search_tag_embeddings
      SET new_tag_id = tags.id
      FROM tags
      WHERE search_tag_embeddings.tag_id = tags.legacy_id
    SQL
    remove_column :search_tag_embeddings, :tag_id
    rename_column :search_tag_embeddings, :new_tag_id, :tag_id
    # Recreate FK constraints
    add_foreign_key :contact_tags, :tags, column: :tag_id
    add_foreign_key :email_message_tags, :tags, column: :tag_id
    add_foreign_key :notification_preferences, :tags, column: :tag_id
    add_foreign_key :search_tag_embeddings, :tags, column: :tag_id

    # ===== contact_email_aliases =====
    # Swap PK
    execute "ALTER TABLE contact_email_aliases DROP CONSTRAINT contact_email_aliases_pkey"
    rename_column :contact_email_aliases, :id, :legacy_id
    rename_column :contact_email_aliases, :uuid_col, :id
    execute "ALTER TABLE contact_email_aliases ADD PRIMARY KEY (id)"

    # ===== connections =====
    # Swap PK
    execute "ALTER TABLE connections DROP CONSTRAINT connections_pkey"
    rename_column :connections, :id, :legacy_id
    rename_column :connections, :uuid_col, :id
    execute "ALTER TABLE connections ADD PRIMARY KEY (id)"

    # ===== calendar_webhook_channels =====
    # Swap PK
    execute "ALTER TABLE calendar_webhook_channels DROP CONSTRAINT calendar_webhook_channels_pkey"
    rename_column :calendar_webhook_channels, :id, :legacy_id
    rename_column :calendar_webhook_channels, :uuid_col, :id
    execute "ALTER TABLE calendar_webhook_channels ADD PRIMARY KEY (id)"

    # ===== calendar_sync_logs =====
    # Swap PK
    execute "ALTER TABLE calendar_sync_logs DROP CONSTRAINT calendar_sync_logs_pkey"
    rename_column :calendar_sync_logs, :id, :legacy_id
    rename_column :calendar_sync_logs, :uuid_col, :id
    execute "ALTER TABLE calendar_sync_logs ADD PRIMARY KEY (id)"

    # ===== calendar_events =====
    # Drop incoming FK constraints
    remove_foreign_key :reminders, :calendar_events, column: :calendar_event_id rescue nil
    # Swap PK
    execute "ALTER TABLE calendar_events DROP CONSTRAINT calendar_events_pkey"
    rename_column :calendar_events, :id, :legacy_id
    rename_column :calendar_events, :uuid_col, :id
    execute "ALTER TABLE calendar_events ADD PRIMARY KEY (id)"
    # Migrate FK columns on referencing tables
    add_column :reminders, :new_calendar_event_id, :uuid
    execute <<~SQL
      UPDATE reminders
      SET new_calendar_event_id = calendar_events.id
      FROM calendar_events
      WHERE reminders.calendar_event_id = calendar_events.legacy_id
    SQL
    remove_column :reminders, :calendar_event_id
    rename_column :reminders, :new_calendar_event_id, :calendar_event_id
    # Recreate FK constraints
    add_foreign_key :reminders, :calendar_events, column: :calendar_event_id

    # ===== email_messages =====
    # Drop incoming FK constraints
    remove_foreign_key :calendar_events, :email_messages, column: :source_email_message_id rescue nil
    remove_foreign_key :document_email_messages, :email_messages, column: :email_message_id rescue nil
    remove_foreign_key :email_message_tags, :email_messages, column: :email_message_id rescue nil
    # Swap PK
    execute "ALTER TABLE email_messages DROP CONSTRAINT email_messages_pkey"
    rename_column :email_messages, :id, :legacy_id
    rename_column :email_messages, :uuid_col, :id
    execute "ALTER TABLE email_messages ADD PRIMARY KEY (id)"
    # Migrate FK columns on referencing tables
    add_column :calendar_events, :new_source_email_message_id, :uuid
    execute <<~SQL
      UPDATE calendar_events
      SET new_source_email_message_id = email_messages.id
      FROM email_messages
      WHERE calendar_events.source_email_message_id = email_messages.legacy_id
    SQL
    remove_column :calendar_events, :source_email_message_id
    rename_column :calendar_events, :new_source_email_message_id, :source_email_message_id
    add_column :document_email_messages, :new_email_message_id, :uuid
    execute <<~SQL
      UPDATE document_email_messages
      SET new_email_message_id = email_messages.id
      FROM email_messages
      WHERE document_email_messages.email_message_id = email_messages.legacy_id
    SQL
    remove_column :document_email_messages, :email_message_id
    rename_column :document_email_messages, :new_email_message_id, :email_message_id
    add_column :email_message_tags, :new_email_message_id, :uuid
    execute <<~SQL
      UPDATE email_message_tags
      SET new_email_message_id = email_messages.id
      FROM email_messages
      WHERE email_message_tags.email_message_id = email_messages.legacy_id
    SQL
    remove_column :email_message_tags, :email_message_id
    rename_column :email_message_tags, :new_email_message_id, :email_message_id
    # Recreate FK constraints
    add_foreign_key :calendar_events, :email_messages, column: :source_email_message_id, on_delete: :nullify
    add_foreign_key :document_email_messages, :email_messages, column: :email_message_id
    add_foreign_key :email_message_tags, :email_messages, column: :email_message_id

    # ===== email_threads =====
    # Drop incoming FK constraints
    remove_foreign_key :email_messages, :email_threads, column: :email_thread_id rescue nil
    # Swap PK
    execute "ALTER TABLE email_threads DROP CONSTRAINT email_threads_pkey"
    rename_column :email_threads, :id, :legacy_id
    rename_column :email_threads, :uuid_col, :id
    execute "ALTER TABLE email_threads ADD PRIMARY KEY (id)"
    # Migrate FK columns on referencing tables
    add_column :email_messages, :new_email_thread_id, :uuid
    execute <<~SQL
      UPDATE email_messages
      SET new_email_thread_id = email_threads.id
      FROM email_threads
      WHERE email_messages.email_thread_id = email_threads.legacy_id
    SQL
    remove_column :email_messages, :email_thread_id
    rename_column :email_messages, :new_email_thread_id, :email_thread_id
    # Recreate FK constraints
    add_foreign_key :email_messages, :email_threads, column: :email_thread_id

    # ===== email_scan_logs =====
    # Drop incoming FK constraints
    remove_foreign_key :email_messages, :email_scan_logs, column: :email_scan_log_id rescue nil
    # Swap PK
    execute "ALTER TABLE email_scan_logs DROP CONSTRAINT email_scan_logs_pkey"
    rename_column :email_scan_logs, :id, :legacy_id
    rename_column :email_scan_logs, :uuid_col, :id
    execute "ALTER TABLE email_scan_logs ADD PRIMARY KEY (id)"
    # Migrate FK columns on referencing tables
    add_column :email_messages, :new_email_scan_log_id, :uuid
    execute <<~SQL
      UPDATE email_messages
      SET new_email_scan_log_id = email_scan_logs.id
      FROM email_scan_logs
      WHERE email_messages.email_scan_log_id = email_scan_logs.legacy_id
    SQL
    remove_column :email_messages, :email_scan_log_id
    rename_column :email_messages, :new_email_scan_log_id, :email_scan_log_id
    # Recreate FK constraints
    add_foreign_key :email_messages, :email_scan_logs, column: :email_scan_log_id

    # ===== contacts =====
    # Drop incoming FK constraints
    remove_foreign_key :contact_email_aliases, :contacts, column: :contact_id rescue nil
    remove_foreign_key :contact_tags, :contacts, column: :contact_id rescue nil
    remove_foreign_key :contacts, :contacts, column: :duplicate_of_id rescue nil
    remove_foreign_key :email_messages, :contacts, column: :contact_id rescue nil
    remove_foreign_key :skim_decisions, :contacts, column: :contact_id rescue nil
    # Swap PK
    execute "ALTER TABLE contacts DROP CONSTRAINT contacts_pkey"
    rename_column :contacts, :id, :legacy_id
    rename_column :contacts, :uuid_col, :id
    execute "ALTER TABLE contacts ADD PRIMARY KEY (id)"
    # Migrate FK columns on referencing tables
    add_column :contact_email_aliases, :new_contact_id, :uuid
    execute <<~SQL
      UPDATE contact_email_aliases
      SET new_contact_id = contacts.id
      FROM contacts
      WHERE contact_email_aliases.contact_id = contacts.legacy_id
    SQL
    remove_column :contact_email_aliases, :contact_id
    rename_column :contact_email_aliases, :new_contact_id, :contact_id
    add_column :contact_tags, :new_contact_id, :uuid
    execute <<~SQL
      UPDATE contact_tags
      SET new_contact_id = contacts.id
      FROM contacts
      WHERE contact_tags.contact_id = contacts.legacy_id
    SQL
    remove_column :contact_tags, :contact_id
    rename_column :contact_tags, :new_contact_id, :contact_id
    add_column :contacts, :new_duplicate_of_id, :uuid
    execute <<~SQL
      UPDATE contacts
      SET new_duplicate_of_id = parent_tbl.id
      FROM contacts AS parent_tbl
      WHERE contacts.duplicate_of_id = parent_tbl.legacy_id
    SQL
    remove_column :contacts, :duplicate_of_id
    rename_column :contacts, :new_duplicate_of_id, :duplicate_of_id
    add_column :email_messages, :new_contact_id, :uuid
    execute <<~SQL
      UPDATE email_messages
      SET new_contact_id = contacts.id
      FROM contacts
      WHERE email_messages.contact_id = contacts.legacy_id
    SQL
    remove_column :email_messages, :contact_id
    rename_column :email_messages, :new_contact_id, :contact_id
    add_column :skim_decisions, :new_contact_id, :uuid
    execute <<~SQL
      UPDATE skim_decisions
      SET new_contact_id = contacts.id
      FROM contacts
      WHERE skim_decisions.contact_id = contacts.legacy_id
    SQL
    remove_column :skim_decisions, :contact_id
    rename_column :skim_decisions, :new_contact_id, :contact_id
    # Recreate FK constraints
    add_foreign_key :contact_email_aliases, :contacts, column: :contact_id
    add_foreign_key :contact_tags, :contacts, column: :contact_id
    add_foreign_key :contacts, :contacts, column: :duplicate_of_id
    add_foreign_key :email_messages, :contacts, column: :contact_id
    add_foreign_key :skim_decisions, :contacts, column: :contact_id

    # ===== people =====
    # Drop incoming FK constraints
    remove_foreign_key :contacts, :people, column: :person_id rescue nil
    remove_foreign_key :contacts, :people, column: :suggested_person_id rescue nil
    # Swap PK
    execute "ALTER TABLE people DROP CONSTRAINT people_pkey"
    rename_column :people, :id, :legacy_id
    rename_column :people, :uuid_col, :id
    execute "ALTER TABLE people ADD PRIMARY KEY (id)"
    # Migrate FK columns on referencing tables
    add_column :contacts, :new_person_id, :uuid
    execute <<~SQL
      UPDATE contacts
      SET new_person_id = people.id
      FROM people
      WHERE contacts.person_id = people.legacy_id
    SQL
    remove_column :contacts, :person_id
    rename_column :contacts, :new_person_id, :person_id
    add_column :contacts, :new_suggested_person_id, :uuid
    execute <<~SQL
      UPDATE contacts
      SET new_suggested_person_id = people.id
      FROM people
      WHERE contacts.suggested_person_id = people.legacy_id
    SQL
    remove_column :contacts, :suggested_person_id
    rename_column :contacts, :new_suggested_person_id, :suggested_person_id
    # Recreate FK constraints
    add_foreign_key :contacts, :people, column: :person_id
    add_foreign_key :contacts, :people, column: :suggested_person_id

    # ===== email_accounts =====
    # Drop incoming FK constraints
    remove_foreign_key :contacts, :email_accounts, column: :email_account_id rescue nil
    remove_foreign_key :documents, :email_accounts, column: :email_account_id rescue nil
    remove_foreign_key :email_account_signatures, :email_accounts, column: :email_account_id rescue nil
    remove_foreign_key :email_account_users, :email_accounts, column: :email_account_id rescue nil
    remove_foreign_key :email_folders, :email_accounts, column: :email_account_id rescue nil
    remove_foreign_key :email_messages, :email_accounts, column: :email_account_id rescue nil
    remove_foreign_key :email_scan_logs, :email_accounts, column: :email_account_id rescue nil
    remove_foreign_key :email_threads, :email_accounts, column: :email_account_id rescue nil
    # Swap PK
    execute "ALTER TABLE email_accounts DROP CONSTRAINT email_accounts_pkey"
    rename_column :email_accounts, :id, :legacy_id
    rename_column :email_accounts, :uuid_col, :id
    execute "ALTER TABLE email_accounts ADD PRIMARY KEY (id)"
    # Migrate FK columns on referencing tables
    add_column :contacts, :new_email_account_id, :uuid
    execute <<~SQL
      UPDATE contacts
      SET new_email_account_id = email_accounts.id
      FROM email_accounts
      WHERE contacts.email_account_id = email_accounts.legacy_id
    SQL
    remove_column :contacts, :email_account_id
    rename_column :contacts, :new_email_account_id, :email_account_id
    add_column :documents, :new_email_account_id, :uuid
    execute <<~SQL
      UPDATE documents
      SET new_email_account_id = email_accounts.id
      FROM email_accounts
      WHERE documents.email_account_id = email_accounts.legacy_id
    SQL
    remove_column :documents, :email_account_id
    rename_column :documents, :new_email_account_id, :email_account_id
    add_column :email_account_signatures, :new_email_account_id, :uuid
    execute <<~SQL
      UPDATE email_account_signatures
      SET new_email_account_id = email_accounts.id
      FROM email_accounts
      WHERE email_account_signatures.email_account_id = email_accounts.legacy_id
    SQL
    remove_column :email_account_signatures, :email_account_id
    rename_column :email_account_signatures, :new_email_account_id, :email_account_id
    add_column :email_account_users, :new_email_account_id, :uuid
    execute <<~SQL
      UPDATE email_account_users
      SET new_email_account_id = email_accounts.id
      FROM email_accounts
      WHERE email_account_users.email_account_id = email_accounts.legacy_id
    SQL
    remove_column :email_account_users, :email_account_id
    rename_column :email_account_users, :new_email_account_id, :email_account_id
    add_column :email_folders, :new_email_account_id, :uuid
    execute <<~SQL
      UPDATE email_folders
      SET new_email_account_id = email_accounts.id
      FROM email_accounts
      WHERE email_folders.email_account_id = email_accounts.legacy_id
    SQL
    remove_column :email_folders, :email_account_id
    rename_column :email_folders, :new_email_account_id, :email_account_id
    add_column :email_messages, :new_email_account_id, :uuid
    execute <<~SQL
      UPDATE email_messages
      SET new_email_account_id = email_accounts.id
      FROM email_accounts
      WHERE email_messages.email_account_id = email_accounts.legacy_id
    SQL
    remove_column :email_messages, :email_account_id
    rename_column :email_messages, :new_email_account_id, :email_account_id
    add_column :email_scan_logs, :new_email_account_id, :uuid
    execute <<~SQL
      UPDATE email_scan_logs
      SET new_email_account_id = email_accounts.id
      FROM email_accounts
      WHERE email_scan_logs.email_account_id = email_accounts.legacy_id
    SQL
    remove_column :email_scan_logs, :email_account_id
    rename_column :email_scan_logs, :new_email_account_id, :email_account_id
    add_column :email_threads, :new_email_account_id, :uuid
    execute <<~SQL
      UPDATE email_threads
      SET new_email_account_id = email_accounts.id
      FROM email_accounts
      WHERE email_threads.email_account_id = email_accounts.legacy_id
    SQL
    remove_column :email_threads, :email_account_id
    rename_column :email_threads, :new_email_account_id, :email_account_id
    # Recreate FK constraints
    add_foreign_key :contacts, :email_accounts, column: :email_account_id
    add_foreign_key :documents, :email_accounts, column: :email_account_id
    add_foreign_key :email_account_signatures, :email_accounts, column: :email_account_id
    add_foreign_key :email_account_users, :email_accounts, column: :email_account_id
    add_foreign_key :email_folders, :email_accounts, column: :email_account_id
    add_foreign_key :email_messages, :email_accounts, column: :email_account_id
    add_foreign_key :email_scan_logs, :email_accounts, column: :email_account_id
    add_foreign_key :email_threads, :email_accounts, column: :email_account_id

    # ===== calendars =====
    # Drop incoming FK constraints
    remove_foreign_key :calendar_events, :calendars, column: :calendar_id rescue nil
    remove_foreign_key :calendar_webhook_channels, :calendars, column: :calendar_id rescue nil
    # Swap PK
    execute "ALTER TABLE calendars DROP CONSTRAINT calendars_pkey"
    rename_column :calendars, :id, :legacy_id
    rename_column :calendars, :uuid_col, :id
    execute "ALTER TABLE calendars ADD PRIMARY KEY (id)"
    # Migrate FK columns on referencing tables
    add_column :calendar_events, :new_calendar_id, :uuid
    execute <<~SQL
      UPDATE calendar_events
      SET new_calendar_id = calendars.id
      FROM calendars
      WHERE calendar_events.calendar_id = calendars.legacy_id
    SQL
    remove_column :calendar_events, :calendar_id
    rename_column :calendar_events, :new_calendar_id, :calendar_id
    add_column :calendar_webhook_channels, :new_calendar_id, :uuid
    execute <<~SQL
      UPDATE calendar_webhook_channels
      SET new_calendar_id = calendars.id
      FROM calendars
      WHERE calendar_webhook_channels.calendar_id = calendars.legacy_id
    SQL
    remove_column :calendar_webhook_channels, :calendar_id
    rename_column :calendar_webhook_channels, :new_calendar_id, :calendar_id
    # Recreate FK constraints
    add_foreign_key :calendar_events, :calendars, column: :calendar_id
    add_foreign_key :calendar_webhook_channels, :calendars, column: :calendar_id

    # ===== calendar_account_users =====
    # Swap PK
    execute "ALTER TABLE calendar_account_users DROP CONSTRAINT calendar_account_users_pkey"
    rename_column :calendar_account_users, :id, :legacy_id
    rename_column :calendar_account_users, :uuid_col, :id
    execute "ALTER TABLE calendar_account_users ADD PRIMARY KEY (id)"

    # ===== calendar_accounts =====
    # Drop incoming FK constraints
    remove_foreign_key :calendar_account_users, :calendar_accounts, column: :calendar_account_id rescue nil
    remove_foreign_key :calendar_sync_logs, :calendar_accounts, column: :calendar_account_id rescue nil
    remove_foreign_key :calendars, :calendar_accounts, column: :calendar_account_id rescue nil
    # Swap PK
    execute "ALTER TABLE calendar_accounts DROP CONSTRAINT calendar_accounts_pkey"
    rename_column :calendar_accounts, :id, :legacy_id
    rename_column :calendar_accounts, :uuid_col, :id
    execute "ALTER TABLE calendar_accounts ADD PRIMARY KEY (id)"
    # Migrate FK columns on referencing tables
    add_column :calendar_account_users, :new_calendar_account_id, :uuid
    execute <<~SQL
      UPDATE calendar_account_users
      SET new_calendar_account_id = calendar_accounts.id
      FROM calendar_accounts
      WHERE calendar_account_users.calendar_account_id = calendar_accounts.legacy_id
    SQL
    remove_column :calendar_account_users, :calendar_account_id
    rename_column :calendar_account_users, :new_calendar_account_id, :calendar_account_id
    add_column :calendar_sync_logs, :new_calendar_account_id, :uuid
    execute <<~SQL
      UPDATE calendar_sync_logs
      SET new_calendar_account_id = calendar_accounts.id
      FROM calendar_accounts
      WHERE calendar_sync_logs.calendar_account_id = calendar_accounts.legacy_id
    SQL
    remove_column :calendar_sync_logs, :calendar_account_id
    rename_column :calendar_sync_logs, :new_calendar_account_id, :calendar_account_id
    add_column :calendars, :new_calendar_account_id, :uuid
    execute <<~SQL
      UPDATE calendars
      SET new_calendar_account_id = calendar_accounts.id
      FROM calendar_accounts
      WHERE calendars.calendar_account_id = calendar_accounts.legacy_id
    SQL
    remove_column :calendars, :calendar_account_id
    rename_column :calendars, :new_calendar_account_id, :calendar_account_id
    # Recreate FK constraints
    add_foreign_key :calendar_account_users, :calendar_accounts, column: :calendar_account_id
    add_foreign_key :calendar_sync_logs, :calendar_accounts, column: :calendar_account_id
    add_foreign_key :calendars, :calendar_accounts, column: :calendar_account_id

    # ===== bug_reports =====
    # Swap PK
    execute "ALTER TABLE bug_reports DROP CONSTRAINT bug_reports_pkey"
    rename_column :bug_reports, :id, :legacy_id
    rename_column :bug_reports, :uuid_col, :id
    execute "ALTER TABLE bug_reports ADD PRIMARY KEY (id)"

    # ===== beta_codes =====
    # Swap PK
    execute "ALTER TABLE beta_codes DROP CONSTRAINT beta_codes_pkey"
    rename_column :beta_codes, :id, :legacy_id
    rename_column :beta_codes, :uuid_col, :id
    execute "ALTER TABLE beta_codes ADD PRIMARY KEY (id)"

    # ===== audit_events =====
    # Swap PK
    execute "ALTER TABLE audit_events DROP CONSTRAINT audit_events_pkey"
    rename_column :audit_events, :id, :legacy_id
    rename_column :audit_events, :uuid_col, :id
    execute "ALTER TABLE audit_events ADD PRIMARY KEY (id)"

    # ===== ai_configurations =====
    # Swap PK
    execute "ALTER TABLE ai_configurations DROP CONSTRAINT ai_configurations_pkey"
    rename_column :ai_configurations, :id, :legacy_id
    rename_column :ai_configurations, :uuid_col, :id
    execute "ALTER TABLE ai_configurations ADD PRIMARY KEY (id)"

    # ===== ai_adapters =====
    # Drop incoming FK constraints
    remove_foreign_key :ai_configurations, :ai_adapters, column: :ai_adapter_id rescue nil
    # Swap PK
    execute "ALTER TABLE ai_adapters DROP CONSTRAINT ai_adapters_pkey"
    rename_column :ai_adapters, :id, :legacy_id
    rename_column :ai_adapters, :uuid_col, :id
    execute "ALTER TABLE ai_adapters ADD PRIMARY KEY (id)"
    # Migrate FK columns on referencing tables
    add_column :ai_configurations, :new_ai_adapter_id, :uuid
    execute <<~SQL
      UPDATE ai_configurations
      SET new_ai_adapter_id = ai_adapters.id
      FROM ai_adapters
      WHERE ai_configurations.ai_adapter_id = ai_adapters.legacy_id
    SQL
    remove_column :ai_configurations, :ai_adapter_id
    rename_column :ai_configurations, :new_ai_adapter_id, :ai_adapter_id
    # Recreate FK constraints
    add_foreign_key :ai_configurations, :ai_adapters, column: :ai_adapter_id

    # ===== agent_messages =====
    # Drop incoming FK constraints
    remove_foreign_key :email_messages, :agent_messages, column: :ai_analysis_message_id rescue nil
    # Swap PK
    execute "ALTER TABLE agent_messages DROP CONSTRAINT agent_messages_pkey"
    rename_column :agent_messages, :id, :legacy_id
    rename_column :agent_messages, :uuid_col, :id
    execute "ALTER TABLE agent_messages ADD PRIMARY KEY (id)"
    # Migrate FK columns on referencing tables
    add_column :email_messages, :new_ai_analysis_message_id, :uuid
    execute <<~SQL
      UPDATE email_messages
      SET new_ai_analysis_message_id = agent_messages.id
      FROM agent_messages
      WHERE email_messages.ai_analysis_message_id = agent_messages.legacy_id
    SQL
    remove_column :email_messages, :ai_analysis_message_id
    rename_column :email_messages, :new_ai_analysis_message_id, :ai_analysis_message_id
    # Recreate FK constraints
    add_foreign_key :email_messages, :agent_messages, column: :ai_analysis_message_id

    # ===== agent_threads =====
    # Drop incoming FK constraints
    remove_foreign_key :agent_messages, :agent_threads, column: :agent_thread_id rescue nil
    remove_foreign_key :thread_follows, :agent_threads, column: :agent_thread_id rescue nil
    # Swap PK
    execute "ALTER TABLE agent_threads DROP CONSTRAINT agent_threads_pkey"
    rename_column :agent_threads, :id, :legacy_id
    rename_column :agent_threads, :uuid_col, :id
    execute "ALTER TABLE agent_threads ADD PRIMARY KEY (id)"
    # Migrate FK columns on referencing tables
    add_column :agent_messages, :new_agent_thread_id, :uuid
    execute <<~SQL
      UPDATE agent_messages
      SET new_agent_thread_id = agent_threads.id
      FROM agent_threads
      WHERE agent_messages.agent_thread_id = agent_threads.legacy_id
    SQL
    remove_column :agent_messages, :agent_thread_id
    rename_column :agent_messages, :new_agent_thread_id, :agent_thread_id
    add_column :thread_follows, :new_agent_thread_id, :uuid
    execute <<~SQL
      UPDATE thread_follows
      SET new_agent_thread_id = agent_threads.id
      FROM agent_threads
      WHERE thread_follows.agent_thread_id = agent_threads.legacy_id
    SQL
    remove_column :thread_follows, :agent_thread_id
    rename_column :thread_follows, :new_agent_thread_id, :agent_thread_id
    # Recreate FK constraints
    add_foreign_key :agent_messages, :agent_threads, column: :agent_thread_id
    add_foreign_key :thread_follows, :agent_threads, column: :agent_thread_id

    # ===== account_exports =====
    # Swap PK
    execute "ALTER TABLE account_exports DROP CONSTRAINT account_exports_pkey"
    rename_column :account_exports, :id, :legacy_id
    rename_column :account_exports, :uuid_col, :id
    execute "ALTER TABLE account_exports ADD PRIMARY KEY (id)"

    # ===== users =====
    # Drop incoming FK constraints
    remove_foreign_key :account_exports, :users, column: :user_id rescue nil
    remove_foreign_key :agent_messages, :users, column: :user_id rescue nil
    remove_foreign_key :agent_threads, :users, column: :user_id rescue nil
    remove_foreign_key :audit_events, :users, column: :user_id rescue nil
    remove_foreign_key :beta_codes, :users, column: :created_by_id rescue nil
    remove_foreign_key :beta_codes, :users, column: :redeemed_by_id rescue nil
    remove_foreign_key :bug_reports, :users, column: :user_id rescue nil
    remove_foreign_key :calendar_account_users, :users, column: :user_id rescue nil
    remove_foreign_key :devices, :users, column: :user_id rescue nil
    remove_foreign_key :documents, :users, column: :reviewed_by_id rescue nil
    remove_foreign_key :email_account_users, :users, column: :user_id rescue nil
    remove_foreign_key :feed_items, :users, column: :user_id rescue nil
    remove_foreign_key :identities, :users, column: :user_id rescue nil
    remove_foreign_key :invitations, :users, column: :accepted_by_id rescue nil
    remove_foreign_key :invitations, :users, column: :invited_by_id rescue nil
    remove_foreign_key :mfa_email_challenges, :users, column: :user_id rescue nil
    remove_foreign_key :notification_preferences, :users, column: :user_id rescue nil
    remove_foreign_key :notifications, :users, column: :user_id rescue nil
    remove_foreign_key :notion_integrations, :users, column: :authorized_by_user_id rescue nil
    remove_foreign_key :oauth_applications, :users, column: :created_by_id rescue nil
    remove_foreign_key :recovery_codes, :users, column: :user_id rescue nil
    remove_foreign_key :reminders, :users, column: :confirmed_by_id rescue nil
    remove_foreign_key :sessions, :users, column: :user_id rescue nil
    remove_foreign_key :signatures, :users, column: :user_id rescue nil
    remove_foreign_key :signup_requests, :users, column: :accepted_by_id rescue nil
    remove_foreign_key :signup_requests, :users, column: :reviewed_by_id rescue nil
    remove_foreign_key :skim_decisions, :users, column: :user_id rescue nil
    remove_foreign_key :thread_follows, :users, column: :user_id rescue nil
    remove_foreign_key :webauthn_credentials, :users, column: :user_id rescue nil
    remove_foreign_key :workflows, :users, column: :created_by_id rescue nil
    # Swap PK
    execute "ALTER TABLE users DROP CONSTRAINT users_pkey"
    rename_column :users, :id, :legacy_id
    rename_column :users, :uuid_col, :id
    execute "ALTER TABLE users ADD PRIMARY KEY (id)"
    # Migrate FK columns on referencing tables
    add_column :account_exports, :new_user_id, :uuid
    execute <<~SQL
      UPDATE account_exports
      SET new_user_id = users.id
      FROM users
      WHERE account_exports.user_id = users.legacy_id
    SQL
    remove_column :account_exports, :user_id
    rename_column :account_exports, :new_user_id, :user_id
    add_column :agent_messages, :new_user_id, :uuid
    execute <<~SQL
      UPDATE agent_messages
      SET new_user_id = users.id
      FROM users
      WHERE agent_messages.user_id = users.legacy_id
    SQL
    remove_column :agent_messages, :user_id
    rename_column :agent_messages, :new_user_id, :user_id
    add_column :agent_threads, :new_user_id, :uuid
    execute <<~SQL
      UPDATE agent_threads
      SET new_user_id = users.id
      FROM users
      WHERE agent_threads.user_id = users.legacy_id
    SQL
    remove_column :agent_threads, :user_id
    rename_column :agent_threads, :new_user_id, :user_id
    add_column :audit_events, :new_user_id, :uuid
    execute <<~SQL
      UPDATE audit_events
      SET new_user_id = users.id
      FROM users
      WHERE audit_events.user_id = users.legacy_id
    SQL
    remove_column :audit_events, :user_id
    rename_column :audit_events, :new_user_id, :user_id
    add_column :beta_codes, :new_created_by_id, :uuid
    execute <<~SQL
      UPDATE beta_codes
      SET new_created_by_id = users.id
      FROM users
      WHERE beta_codes.created_by_id = users.legacy_id
    SQL
    remove_column :beta_codes, :created_by_id
    rename_column :beta_codes, :new_created_by_id, :created_by_id
    add_column :beta_codes, :new_redeemed_by_id, :uuid
    execute <<~SQL
      UPDATE beta_codes
      SET new_redeemed_by_id = users.id
      FROM users
      WHERE beta_codes.redeemed_by_id = users.legacy_id
    SQL
    remove_column :beta_codes, :redeemed_by_id
    rename_column :beta_codes, :new_redeemed_by_id, :redeemed_by_id
    add_column :bug_reports, :new_user_id, :uuid
    execute <<~SQL
      UPDATE bug_reports
      SET new_user_id = users.id
      FROM users
      WHERE bug_reports.user_id = users.legacy_id
    SQL
    remove_column :bug_reports, :user_id
    rename_column :bug_reports, :new_user_id, :user_id
    add_column :calendar_account_users, :new_user_id, :uuid
    execute <<~SQL
      UPDATE calendar_account_users
      SET new_user_id = users.id
      FROM users
      WHERE calendar_account_users.user_id = users.legacy_id
    SQL
    remove_column :calendar_account_users, :user_id
    rename_column :calendar_account_users, :new_user_id, :user_id
    add_column :devices, :new_user_id, :uuid
    execute <<~SQL
      UPDATE devices
      SET new_user_id = users.id
      FROM users
      WHERE devices.user_id = users.legacy_id
    SQL
    remove_column :devices, :user_id
    rename_column :devices, :new_user_id, :user_id
    add_column :documents, :new_reviewed_by_id, :uuid
    execute <<~SQL
      UPDATE documents
      SET new_reviewed_by_id = users.id
      FROM users
      WHERE documents.reviewed_by_id = users.legacy_id
    SQL
    remove_column :documents, :reviewed_by_id
    rename_column :documents, :new_reviewed_by_id, :reviewed_by_id
    add_column :email_account_users, :new_user_id, :uuid
    execute <<~SQL
      UPDATE email_account_users
      SET new_user_id = users.id
      FROM users
      WHERE email_account_users.user_id = users.legacy_id
    SQL
    remove_column :email_account_users, :user_id
    rename_column :email_account_users, :new_user_id, :user_id
    add_column :feed_items, :new_user_id, :uuid
    execute <<~SQL
      UPDATE feed_items
      SET new_user_id = users.id
      FROM users
      WHERE feed_items.user_id = users.legacy_id
    SQL
    remove_column :feed_items, :user_id
    rename_column :feed_items, :new_user_id, :user_id
    add_column :identities, :new_user_id, :uuid
    execute <<~SQL
      UPDATE identities
      SET new_user_id = users.id
      FROM users
      WHERE identities.user_id = users.legacy_id
    SQL
    remove_column :identities, :user_id
    rename_column :identities, :new_user_id, :user_id
    add_column :invitations, :new_accepted_by_id, :uuid
    execute <<~SQL
      UPDATE invitations
      SET new_accepted_by_id = users.id
      FROM users
      WHERE invitations.accepted_by_id = users.legacy_id
    SQL
    remove_column :invitations, :accepted_by_id
    rename_column :invitations, :new_accepted_by_id, :accepted_by_id
    add_column :invitations, :new_invited_by_id, :uuid
    execute <<~SQL
      UPDATE invitations
      SET new_invited_by_id = users.id
      FROM users
      WHERE invitations.invited_by_id = users.legacy_id
    SQL
    remove_column :invitations, :invited_by_id
    rename_column :invitations, :new_invited_by_id, :invited_by_id
    add_column :mfa_email_challenges, :new_user_id, :uuid
    execute <<~SQL
      UPDATE mfa_email_challenges
      SET new_user_id = users.id
      FROM users
      WHERE mfa_email_challenges.user_id = users.legacy_id
    SQL
    remove_column :mfa_email_challenges, :user_id
    rename_column :mfa_email_challenges, :new_user_id, :user_id
    add_column :notification_preferences, :new_user_id, :uuid
    execute <<~SQL
      UPDATE notification_preferences
      SET new_user_id = users.id
      FROM users
      WHERE notification_preferences.user_id = users.legacy_id
    SQL
    remove_column :notification_preferences, :user_id
    rename_column :notification_preferences, :new_user_id, :user_id
    add_column :notifications, :new_user_id, :uuid
    execute <<~SQL
      UPDATE notifications
      SET new_user_id = users.id
      FROM users
      WHERE notifications.user_id = users.legacy_id
    SQL
    remove_column :notifications, :user_id
    rename_column :notifications, :new_user_id, :user_id
    add_column :notion_integrations, :new_authorized_by_user_id, :uuid
    execute <<~SQL
      UPDATE notion_integrations
      SET new_authorized_by_user_id = users.id
      FROM users
      WHERE notion_integrations.authorized_by_user_id = users.legacy_id
    SQL
    remove_column :notion_integrations, :authorized_by_user_id
    rename_column :notion_integrations, :new_authorized_by_user_id, :authorized_by_user_id
    add_column :oauth_applications, :new_created_by_id, :uuid
    execute <<~SQL
      UPDATE oauth_applications
      SET new_created_by_id = users.id
      FROM users
      WHERE oauth_applications.created_by_id = users.legacy_id
    SQL
    remove_column :oauth_applications, :created_by_id
    rename_column :oauth_applications, :new_created_by_id, :created_by_id
    add_column :recovery_codes, :new_user_id, :uuid
    execute <<~SQL
      UPDATE recovery_codes
      SET new_user_id = users.id
      FROM users
      WHERE recovery_codes.user_id = users.legacy_id
    SQL
    remove_column :recovery_codes, :user_id
    rename_column :recovery_codes, :new_user_id, :user_id
    add_column :reminders, :new_confirmed_by_id, :uuid
    execute <<~SQL
      UPDATE reminders
      SET new_confirmed_by_id = users.id
      FROM users
      WHERE reminders.confirmed_by_id = users.legacy_id
    SQL
    remove_column :reminders, :confirmed_by_id
    rename_column :reminders, :new_confirmed_by_id, :confirmed_by_id
    add_column :sessions, :new_user_id, :uuid
    execute <<~SQL
      UPDATE sessions
      SET new_user_id = users.id
      FROM users
      WHERE sessions.user_id = users.legacy_id
    SQL
    remove_column :sessions, :user_id
    rename_column :sessions, :new_user_id, :user_id
    add_column :signatures, :new_user_id, :uuid
    execute <<~SQL
      UPDATE signatures
      SET new_user_id = users.id
      FROM users
      WHERE signatures.user_id = users.legacy_id
    SQL
    remove_column :signatures, :user_id
    rename_column :signatures, :new_user_id, :user_id
    add_column :signup_requests, :new_accepted_by_id, :uuid
    execute <<~SQL
      UPDATE signup_requests
      SET new_accepted_by_id = users.id
      FROM users
      WHERE signup_requests.accepted_by_id = users.legacy_id
    SQL
    remove_column :signup_requests, :accepted_by_id
    rename_column :signup_requests, :new_accepted_by_id, :accepted_by_id
    add_column :signup_requests, :new_reviewed_by_id, :uuid
    execute <<~SQL
      UPDATE signup_requests
      SET new_reviewed_by_id = users.id
      FROM users
      WHERE signup_requests.reviewed_by_id = users.legacy_id
    SQL
    remove_column :signup_requests, :reviewed_by_id
    rename_column :signup_requests, :new_reviewed_by_id, :reviewed_by_id
    add_column :skim_decisions, :new_user_id, :uuid
    execute <<~SQL
      UPDATE skim_decisions
      SET new_user_id = users.id
      FROM users
      WHERE skim_decisions.user_id = users.legacy_id
    SQL
    remove_column :skim_decisions, :user_id
    rename_column :skim_decisions, :new_user_id, :user_id
    add_column :thread_follows, :new_user_id, :uuid
    execute <<~SQL
      UPDATE thread_follows
      SET new_user_id = users.id
      FROM users
      WHERE thread_follows.user_id = users.legacy_id
    SQL
    remove_column :thread_follows, :user_id
    rename_column :thread_follows, :new_user_id, :user_id
    add_column :webauthn_credentials, :new_user_id, :uuid
    execute <<~SQL
      UPDATE webauthn_credentials
      SET new_user_id = users.id
      FROM users
      WHERE webauthn_credentials.user_id = users.legacy_id
    SQL
    remove_column :webauthn_credentials, :user_id
    rename_column :webauthn_credentials, :new_user_id, :user_id
    add_column :workflows, :new_created_by_id, :uuid
    execute <<~SQL
      UPDATE workflows
      SET new_created_by_id = users.id
      FROM users
      WHERE workflows.created_by_id = users.legacy_id
    SQL
    remove_column :workflows, :created_by_id
    rename_column :workflows, :new_created_by_id, :created_by_id
    # Recreate FK constraints
    add_foreign_key :account_exports, :users, column: :user_id
    add_foreign_key :agent_messages, :users, column: :user_id
    add_foreign_key :agent_threads, :users, column: :user_id
    add_foreign_key :audit_events, :users, column: :user_id, on_delete: :nullify
    add_foreign_key :beta_codes, :users, column: :created_by_id
    add_foreign_key :beta_codes, :users, column: :redeemed_by_id
    add_foreign_key :bug_reports, :users, column: :user_id
    add_foreign_key :calendar_account_users, :users, column: :user_id
    add_foreign_key :devices, :users, column: :user_id
    add_foreign_key :documents, :users, column: :reviewed_by_id
    add_foreign_key :email_account_users, :users, column: :user_id
    add_foreign_key :feed_items, :users, column: :user_id
    add_foreign_key :identities, :users, column: :user_id
    add_foreign_key :invitations, :users, column: :accepted_by_id
    add_foreign_key :invitations, :users, column: :invited_by_id
    add_foreign_key :mfa_email_challenges, :users, column: :user_id
    add_foreign_key :notification_preferences, :users, column: :user_id
    add_foreign_key :notifications, :users, column: :user_id
    add_foreign_key :notion_integrations, :users, column: :authorized_by_user_id
    add_foreign_key :oauth_applications, :users, column: :created_by_id
    add_foreign_key :recovery_codes, :users, column: :user_id
    add_foreign_key :reminders, :users, column: :confirmed_by_id
    add_foreign_key :sessions, :users, column: :user_id
    add_foreign_key :signatures, :users, column: :user_id
    add_foreign_key :signup_requests, :users, column: :accepted_by_id
    add_foreign_key :signup_requests, :users, column: :reviewed_by_id
    add_foreign_key :skim_decisions, :users, column: :user_id
    add_foreign_key :thread_follows, :users, column: :user_id
    add_foreign_key :webauthn_credentials, :users, column: :user_id
    add_foreign_key :workflows, :users, column: :created_by_id

    # ===== workspaces =====
    # Drop incoming FK constraints
    remove_foreign_key :agent_threads, :workspaces, column: :workspace_id rescue nil
    remove_foreign_key :ai_adapters, :workspaces, column: :workspace_id rescue nil
    remove_foreign_key :ai_configurations, :workspaces, column: :workspace_id rescue nil
    remove_foreign_key :bug_reports, :workspaces, column: :workspace_id rescue nil
    remove_foreign_key :calendar_accounts, :workspaces, column: :workspace_id rescue nil
    remove_foreign_key :connections, :workspaces, column: :workspace_id rescue nil
    remove_foreign_key :contacts, :workspaces, column: :workspace_id rescue nil
    remove_foreign_key :document_types, :workspaces, column: :workspace_id rescue nil
    remove_foreign_key :documents, :workspaces, column: :workspace_id rescue nil
    remove_foreign_key :email_accounts, :workspaces, column: :workspace_id rescue nil
    remove_foreign_key :events, :workspaces, column: :workspace_id rescue nil
    remove_foreign_key :exports, :workspaces, column: :workspace_id rescue nil
    remove_foreign_key :feed_items, :workspaces, column: :workspace_id rescue nil
    remove_foreign_key :google_drive_accounts, :workspaces, column: :workspace_id rescue nil
    remove_foreign_key :invitations, :workspaces, column: :workspace_id rescue nil
    remove_foreign_key :mail_folders, :workspaces, column: :workspace_id rescue nil
    remove_foreign_key :notion_integrations, :workspaces, column: :workspace_id rescue nil
    remove_foreign_key :oauth_applications, :workspaces, column: :workspace_id rescue nil
    remove_foreign_key :people, :workspaces, column: :workspace_id rescue nil
    remove_foreign_key :reminders, :workspaces, column: :workspace_id rescue nil
    remove_foreign_key :search_chunks, :workspaces, column: :workspace_id rescue nil
    remove_foreign_key :search_records, :workspaces, column: :workspace_id rescue nil
    remove_foreign_key :search_tag_embeddings, :workspaces, column: :workspace_id rescue nil
    remove_foreign_key :skim_decisions, :workspaces, column: :workspace_id rescue nil
    remove_foreign_key :tags, :workspaces, column: :workspace_id rescue nil
    remove_foreign_key :users, :workspaces, column: :workspace_id rescue nil
    remove_foreign_key :workflow_executions, :workspaces, column: :workspace_id rescue nil
    remove_foreign_key :workflows, :workspaces, column: :workspace_id rescue nil
    remove_foreign_key :zoho_drive_accounts, :workspaces, column: :workspace_id rescue nil
    # Swap PK
    execute "ALTER TABLE workspaces DROP CONSTRAINT workspaces_pkey"
    rename_column :workspaces, :id, :legacy_id
    rename_column :workspaces, :uuid_col, :id
    execute "ALTER TABLE workspaces ADD PRIMARY KEY (id)"
    # Migrate FK columns on referencing tables
    add_column :agent_threads, :new_workspace_id, :uuid
    execute <<~SQL
      UPDATE agent_threads
      SET new_workspace_id = workspaces.id
      FROM workspaces
      WHERE agent_threads.workspace_id = workspaces.legacy_id
    SQL
    remove_column :agent_threads, :workspace_id
    rename_column :agent_threads, :new_workspace_id, :workspace_id
    add_column :ai_adapters, :new_workspace_id, :uuid
    execute <<~SQL
      UPDATE ai_adapters
      SET new_workspace_id = workspaces.id
      FROM workspaces
      WHERE ai_adapters.workspace_id = workspaces.legacy_id
    SQL
    remove_column :ai_adapters, :workspace_id
    rename_column :ai_adapters, :new_workspace_id, :workspace_id
    add_column :ai_configurations, :new_workspace_id, :uuid
    execute <<~SQL
      UPDATE ai_configurations
      SET new_workspace_id = workspaces.id
      FROM workspaces
      WHERE ai_configurations.workspace_id = workspaces.legacy_id
    SQL
    remove_column :ai_configurations, :workspace_id
    rename_column :ai_configurations, :new_workspace_id, :workspace_id
    add_column :bug_reports, :new_workspace_id, :uuid
    execute <<~SQL
      UPDATE bug_reports
      SET new_workspace_id = workspaces.id
      FROM workspaces
      WHERE bug_reports.workspace_id = workspaces.legacy_id
    SQL
    remove_column :bug_reports, :workspace_id
    rename_column :bug_reports, :new_workspace_id, :workspace_id
    add_column :calendar_accounts, :new_workspace_id, :uuid
    execute <<~SQL
      UPDATE calendar_accounts
      SET new_workspace_id = workspaces.id
      FROM workspaces
      WHERE calendar_accounts.workspace_id = workspaces.legacy_id
    SQL
    remove_column :calendar_accounts, :workspace_id
    rename_column :calendar_accounts, :new_workspace_id, :workspace_id
    add_column :connections, :new_workspace_id, :uuid
    execute <<~SQL
      UPDATE connections
      SET new_workspace_id = workspaces.id
      FROM workspaces
      WHERE connections.workspace_id = workspaces.legacy_id
    SQL
    remove_column :connections, :workspace_id
    rename_column :connections, :new_workspace_id, :workspace_id
    add_column :contacts, :new_workspace_id, :uuid
    execute <<~SQL
      UPDATE contacts
      SET new_workspace_id = workspaces.id
      FROM workspaces
      WHERE contacts.workspace_id = workspaces.legacy_id
    SQL
    remove_column :contacts, :workspace_id
    rename_column :contacts, :new_workspace_id, :workspace_id
    add_column :document_types, :new_workspace_id, :uuid
    execute <<~SQL
      UPDATE document_types
      SET new_workspace_id = workspaces.id
      FROM workspaces
      WHERE document_types.workspace_id = workspaces.legacy_id
    SQL
    remove_column :document_types, :workspace_id
    rename_column :document_types, :new_workspace_id, :workspace_id
    add_column :documents, :new_workspace_id, :uuid
    execute <<~SQL
      UPDATE documents
      SET new_workspace_id = workspaces.id
      FROM workspaces
      WHERE documents.workspace_id = workspaces.legacy_id
    SQL
    remove_column :documents, :workspace_id
    rename_column :documents, :new_workspace_id, :workspace_id
    add_column :email_accounts, :new_workspace_id, :uuid
    execute <<~SQL
      UPDATE email_accounts
      SET new_workspace_id = workspaces.id
      FROM workspaces
      WHERE email_accounts.workspace_id = workspaces.legacy_id
    SQL
    remove_column :email_accounts, :workspace_id
    rename_column :email_accounts, :new_workspace_id, :workspace_id
    add_column :events, :new_workspace_id, :uuid
    execute <<~SQL
      UPDATE events
      SET new_workspace_id = workspaces.id
      FROM workspaces
      WHERE events.workspace_id = workspaces.legacy_id
    SQL
    remove_column :events, :workspace_id
    rename_column :events, :new_workspace_id, :workspace_id
    add_column :exports, :new_workspace_id, :uuid
    execute <<~SQL
      UPDATE exports
      SET new_workspace_id = workspaces.id
      FROM workspaces
      WHERE exports.workspace_id = workspaces.legacy_id
    SQL
    remove_column :exports, :workspace_id
    rename_column :exports, :new_workspace_id, :workspace_id
    add_column :feed_items, :new_workspace_id, :uuid
    execute <<~SQL
      UPDATE feed_items
      SET new_workspace_id = workspaces.id
      FROM workspaces
      WHERE feed_items.workspace_id = workspaces.legacy_id
    SQL
    remove_column :feed_items, :workspace_id
    rename_column :feed_items, :new_workspace_id, :workspace_id
    add_column :google_drive_accounts, :new_workspace_id, :uuid
    execute <<~SQL
      UPDATE google_drive_accounts
      SET new_workspace_id = workspaces.id
      FROM workspaces
      WHERE google_drive_accounts.workspace_id = workspaces.legacy_id
    SQL
    remove_column :google_drive_accounts, :workspace_id
    rename_column :google_drive_accounts, :new_workspace_id, :workspace_id
    add_column :invitations, :new_workspace_id, :uuid
    execute <<~SQL
      UPDATE invitations
      SET new_workspace_id = workspaces.id
      FROM workspaces
      WHERE invitations.workspace_id = workspaces.legacy_id
    SQL
    remove_column :invitations, :workspace_id
    rename_column :invitations, :new_workspace_id, :workspace_id
    add_column :mail_folders, :new_workspace_id, :uuid
    execute <<~SQL
      UPDATE mail_folders
      SET new_workspace_id = workspaces.id
      FROM workspaces
      WHERE mail_folders.workspace_id = workspaces.legacy_id
    SQL
    remove_column :mail_folders, :workspace_id
    rename_column :mail_folders, :new_workspace_id, :workspace_id
    add_column :notion_integrations, :new_workspace_id, :uuid
    execute <<~SQL
      UPDATE notion_integrations
      SET new_workspace_id = workspaces.id
      FROM workspaces
      WHERE notion_integrations.workspace_id = workspaces.legacy_id
    SQL
    remove_column :notion_integrations, :workspace_id
    rename_column :notion_integrations, :new_workspace_id, :workspace_id
    add_column :oauth_applications, :new_workspace_id, :uuid
    execute <<~SQL
      UPDATE oauth_applications
      SET new_workspace_id = workspaces.id
      FROM workspaces
      WHERE oauth_applications.workspace_id = workspaces.legacy_id
    SQL
    remove_column :oauth_applications, :workspace_id
    rename_column :oauth_applications, :new_workspace_id, :workspace_id
    add_column :people, :new_workspace_id, :uuid
    execute <<~SQL
      UPDATE people
      SET new_workspace_id = workspaces.id
      FROM workspaces
      WHERE people.workspace_id = workspaces.legacy_id
    SQL
    remove_column :people, :workspace_id
    rename_column :people, :new_workspace_id, :workspace_id
    add_column :reminders, :new_workspace_id, :uuid
    execute <<~SQL
      UPDATE reminders
      SET new_workspace_id = workspaces.id
      FROM workspaces
      WHERE reminders.workspace_id = workspaces.legacy_id
    SQL
    remove_column :reminders, :workspace_id
    rename_column :reminders, :new_workspace_id, :workspace_id
    add_column :search_chunks, :new_workspace_id, :uuid
    execute <<~SQL
      UPDATE search_chunks
      SET new_workspace_id = workspaces.id
      FROM workspaces
      WHERE search_chunks.workspace_id = workspaces.legacy_id
    SQL
    remove_column :search_chunks, :workspace_id
    rename_column :search_chunks, :new_workspace_id, :workspace_id
    add_column :search_records, :new_workspace_id, :uuid
    execute <<~SQL
      UPDATE search_records
      SET new_workspace_id = workspaces.id
      FROM workspaces
      WHERE search_records.workspace_id = workspaces.legacy_id
    SQL
    remove_column :search_records, :workspace_id
    rename_column :search_records, :new_workspace_id, :workspace_id
    add_column :search_tag_embeddings, :new_workspace_id, :uuid
    execute <<~SQL
      UPDATE search_tag_embeddings
      SET new_workspace_id = workspaces.id
      FROM workspaces
      WHERE search_tag_embeddings.workspace_id = workspaces.legacy_id
    SQL
    remove_column :search_tag_embeddings, :workspace_id
    rename_column :search_tag_embeddings, :new_workspace_id, :workspace_id
    add_column :skim_decisions, :new_workspace_id, :uuid
    execute <<~SQL
      UPDATE skim_decisions
      SET new_workspace_id = workspaces.id
      FROM workspaces
      WHERE skim_decisions.workspace_id = workspaces.legacy_id
    SQL
    remove_column :skim_decisions, :workspace_id
    rename_column :skim_decisions, :new_workspace_id, :workspace_id
    add_column :tags, :new_workspace_id, :uuid
    execute <<~SQL
      UPDATE tags
      SET new_workspace_id = workspaces.id
      FROM workspaces
      WHERE tags.workspace_id = workspaces.legacy_id
    SQL
    remove_column :tags, :workspace_id
    rename_column :tags, :new_workspace_id, :workspace_id
    add_column :users, :new_workspace_id, :uuid
    execute <<~SQL
      UPDATE users
      SET new_workspace_id = workspaces.id
      FROM workspaces
      WHERE users.workspace_id = workspaces.legacy_id
    SQL
    remove_column :users, :workspace_id
    rename_column :users, :new_workspace_id, :workspace_id
    add_column :workflow_executions, :new_workspace_id, :uuid
    execute <<~SQL
      UPDATE workflow_executions
      SET new_workspace_id = workspaces.id
      FROM workspaces
      WHERE workflow_executions.workspace_id = workspaces.legacy_id
    SQL
    remove_column :workflow_executions, :workspace_id
    rename_column :workflow_executions, :new_workspace_id, :workspace_id
    add_column :workflows, :new_workspace_id, :uuid
    execute <<~SQL
      UPDATE workflows
      SET new_workspace_id = workspaces.id
      FROM workspaces
      WHERE workflows.workspace_id = workspaces.legacy_id
    SQL
    remove_column :workflows, :workspace_id
    rename_column :workflows, :new_workspace_id, :workspace_id
    add_column :zoho_drive_accounts, :new_workspace_id, :uuid
    execute <<~SQL
      UPDATE zoho_drive_accounts
      SET new_workspace_id = workspaces.id
      FROM workspaces
      WHERE zoho_drive_accounts.workspace_id = workspaces.legacy_id
    SQL
    remove_column :zoho_drive_accounts, :workspace_id
    rename_column :zoho_drive_accounts, :new_workspace_id, :workspace_id
    # Recreate FK constraints
    add_foreign_key :agent_threads, :workspaces, column: :workspace_id
    add_foreign_key :ai_adapters, :workspaces, column: :workspace_id
    add_foreign_key :ai_configurations, :workspaces, column: :workspace_id
    add_foreign_key :bug_reports, :workspaces, column: :workspace_id
    add_foreign_key :calendar_accounts, :workspaces, column: :workspace_id
    add_foreign_key :connections, :workspaces, column: :workspace_id
    add_foreign_key :contacts, :workspaces, column: :workspace_id
    add_foreign_key :document_types, :workspaces, column: :workspace_id
    add_foreign_key :documents, :workspaces, column: :workspace_id
    add_foreign_key :email_accounts, :workspaces, column: :workspace_id
    add_foreign_key :events, :workspaces, column: :workspace_id
    add_foreign_key :exports, :workspaces, column: :workspace_id
    add_foreign_key :feed_items, :workspaces, column: :workspace_id
    add_foreign_key :google_drive_accounts, :workspaces, column: :workspace_id
    add_foreign_key :invitations, :workspaces, column: :workspace_id
    add_foreign_key :mail_folders, :workspaces, column: :workspace_id
    add_foreign_key :notion_integrations, :workspaces, column: :workspace_id
    add_foreign_key :oauth_applications, :workspaces, column: :workspace_id
    add_foreign_key :people, :workspaces, column: :workspace_id
    add_foreign_key :reminders, :workspaces, column: :workspace_id
    add_foreign_key :search_chunks, :workspaces, column: :workspace_id
    add_foreign_key :search_records, :workspaces, column: :workspace_id
    add_foreign_key :search_tag_embeddings, :workspaces, column: :workspace_id
    add_foreign_key :skim_decisions, :workspaces, column: :workspace_id
    add_foreign_key :tags, :workspaces, column: :workspace_id
    add_foreign_key :users, :workspaces, column: :workspace_id
    add_foreign_key :workflow_executions, :workspaces, column: :workspace_id
    add_foreign_key :workflows, :workspaces, column: :workspace_id
    add_foreign_key :zoho_drive_accounts, :workspaces, column: :workspace_id

    # ===== Polymorphic columns: change type (no backfill — dev DB only) =====
    # These columns store references to multiple tables. After PK swap,
    # existing integer values become dangling. For production, backfill
    # per polymorphic type would be needed.
    change_column :agent_threads, :contextable_id, :text
    change_column :audit_events, :target_id, :text
    change_column :events, :actor_id, :text
    change_column :events, :subject_id, :text
    change_column :feed_items, :subject_id, :text
    change_column :folder_memberships, :folderable_id, :text
    change_column :notifications, :notifiable_id, :text
    change_column :reminders, :source_id, :text
    change_column :search_chunks, :searchable_id, :text
    change_column :search_records, :searchable_id, :text
  end

  def down
    raise ActiveRecord::IrreversibleMigration
  end
end

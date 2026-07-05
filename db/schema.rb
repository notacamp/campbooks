# This file is auto-generated from the current state of the database. Instead
# of editing this file, please use the migrations feature of Active Record to
# incrementally modify your database, and then regenerate this schema definition.
#
# This file is the source Rails uses to define your schema when running `bin/rails
# db:schema:load`. When creating a new database, `bin/rails db:schema:load` tends to
# be faster and is potentially less error prone than running all of your
# migrations from scratch. Old migrations may fail to apply correctly if those
# migrations use external dependencies or application code.
#
# It's strongly recommended that you check this file into your version control system.

ActiveRecord::Schema[8.1].define(version: 2026_07_06_100000) do
  # These are extensions that must be enabled in order to support this database
  enable_extension "pg_catalog.plpgsql"
  enable_extension "vector"

  create_table "account_exports", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.integer "status", default: 0, null: false
    t.datetime "updated_at", null: false
    t.uuid "user_id", null: false
    t.index ["user_id"], name: "index_account_exports_on_user_id"
  end

  create_table "action_text_rich_texts", force: :cascade do |t|
    t.text "body"
    t.datetime "created_at", null: false
    t.string "name", null: false
    t.uuid "record_id", null: false
    t.string "record_type", null: false
    t.datetime "updated_at", null: false
    t.index ["record_type", "record_id", "name"], name: "index_action_text_rich_texts_uniqueness", unique: true
  end

  create_table "active_storage_attachments", force: :cascade do |t|
    t.bigint "blob_id", null: false
    t.datetime "created_at", null: false
    t.string "name", null: false
    t.uuid "record_id", null: false
    t.string "record_type", null: false
    t.index ["blob_id"], name: "index_active_storage_attachments_on_blob_id"
    t.index ["record_type", "record_id", "name", "blob_id"], name: "index_active_storage_attachments_uniqueness", unique: true
  end

  create_table "active_storage_blobs", force: :cascade do |t|
    t.bigint "byte_size", null: false
    t.string "checksum"
    t.string "content_type"
    t.datetime "created_at", null: false
    t.string "filename", null: false
    t.string "key", null: false
    t.text "metadata"
    t.string "service_name", null: false
    t.index ["key"], name: "index_active_storage_blobs_on_key", unique: true
  end

  create_table "active_storage_variant_records", force: :cascade do |t|
    t.bigint "blob_id", null: false
    t.string "variation_digest", null: false
    t.index ["blob_id", "variation_digest"], name: "index_active_storage_variant_records_uniqueness", unique: true
  end

  create_table "agent_messages", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.uuid "agent_thread_id"
    t.jsonb "ai_auto_actions", default: [], null: false
    t.jsonb "ai_prompts", default: [], null: false
    t.jsonb "ai_provenance", default: {}, null: false
    t.jsonb "ai_suggested_actions", default: [], null: false
    t.text "ai_thinking"
    t.integer "author_type", default: 0, null: false
    t.text "content", null: false
    t.datetime "created_at", null: false
    t.boolean "draft", default: false, null: false
    t.boolean "outdated", default: false, null: false
    t.integer "reply_status"
    t.jsonb "steps", default: [], null: false
    t.datetime "updated_at", null: false
    t.uuid "user_id", null: false
    t.datetime "viewed_at"
    t.index ["agent_thread_id", "created_at"], name: "index_agent_messages_on_agent_thread_id_and_created_at"
    t.index ["agent_thread_id"], name: "index_agent_messages_on_agent_thread_id"
    t.index ["agent_thread_id"], name: "index_agent_messages_on_thread_unviewed", where: "(viewed_at IS NULL)"
    t.index ["user_id", "created_at"], name: "index_agent_messages_on_user_id_and_created_at"
    t.index ["user_id"], name: "index_agent_messages_on_user_id"
  end

  create_table "agent_threads", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.uuid "contextable_id"
    t.string "contextable_type"
    t.datetime "created_at", null: false
    t.integer "purpose", default: 0, null: false
    t.string "title", null: false
    t.datetime "updated_at", null: false
    t.uuid "user_id", null: false
    t.uuid "workspace_id"
    t.index ["contextable_type", "contextable_id"], name: "index_agent_threads_on_contextable_type_and_contextable_id"
    t.index ["purpose"], name: "index_agent_threads_on_purpose"
    t.index ["user_id", "created_at"], name: "index_agent_threads_on_user_id_and_created_at"
    t.index ["user_id"], name: "index_agent_threads_on_user_id"
    t.index ["workspace_id"], name: "index_agent_threads_on_workspace_id"
  end

  create_table "ai_adapters", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.string "api_key"
    t.datetime "created_at", null: false
    t.boolean "enabled", default: true, null: false
    t.string "endpoint_url"
    t.jsonb "extra_settings", default: {}, null: false
    t.boolean "managed", default: false, null: false
    t.string "name", null: false
    t.string "provider", null: false
    t.datetime "updated_at", null: false
    t.uuid "workspace_id", null: false
    t.index ["workspace_id", "name"], name: "index_ai_adapters_on_workspace_id_and_name", unique: true
    t.index ["workspace_id"], name: "index_ai_adapters_on_workspace_id"
  end

  create_table "ai_configurations", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.uuid "ai_adapter_id"
    t.datetime "created_at", null: false
    t.boolean "enabled", default: true, null: false
    t.integer "max_tokens", default: 4000, null: false
    t.string "model", default: "", null: false
    t.string "purpose", null: false
    t.text "system_prompt"
    t.float "temperature", default: 0.0, null: false
    t.datetime "updated_at", null: false
    t.uuid "workspace_id"
    t.index ["ai_adapter_id"], name: "index_ai_configurations_on_ai_adapter_id"
    t.index ["workspace_id", "purpose"], name: "index_ai_configurations_on_workspace_and_purpose", unique: true
    t.index ["workspace_id"], name: "index_ai_configurations_on_workspace_id"
  end

  create_table "ai_prompts", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.text "instructions"
    t.string "purpose", null: false
    t.datetime "updated_at", null: false
    t.uuid "workspace_id", null: false
    t.index ["workspace_id", "purpose"], name: "index_ai_prompts_on_workspace_id_and_purpose", unique: true
    t.index ["workspace_id"], name: "index_ai_prompts_on_workspace_id"
  end

  create_table "audit_events", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.string "action"
    t.datetime "created_at", null: false
    t.string "ip_address"
    t.jsonb "metadata", default: {}, null: false
    t.uuid "target_id"
    t.string "target_type"
    t.datetime "updated_at", null: false
    t.string "user_agent"
    t.uuid "user_id"
    t.index ["action"], name: "index_audit_events_on_action"
    t.index ["target_type", "target_id"], name: "index_audit_events_on_target"
    t.index ["user_id", "created_at"], name: "index_audit_events_on_user_id_and_created_at"
    t.index ["user_id"], name: "index_audit_events_on_user_id"
  end

  create_table "authored_documents", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.uuid "author_id"
    t.datetime "created_at", null: false
    t.text "html_content"
    t.string "title", null: false
    t.datetime "updated_at", null: false
    t.uuid "workspace_id", null: false
    t.index ["author_id"], name: "index_authored_documents_on_author_id"
    t.index ["workspace_id", "created_at"], name: "index_authored_documents_on_workspace_id_and_created_at"
    t.index ["workspace_id"], name: "index_authored_documents_on_workspace_id"
  end

  create_table "beta_codes", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.string "code", null: false
    t.datetime "created_at", null: false
    t.uuid "created_by_id"
    t.datetime "expires_at"
    t.string "label"
    t.datetime "redeemed_at"
    t.uuid "redeemed_by_id"
    t.datetime "updated_at", null: false
    t.index ["code"], name: "index_beta_codes_on_code", unique: true
    t.index ["created_by_id"], name: "index_beta_codes_on_created_by_id"
    t.index ["redeemed_by_id"], name: "index_beta_codes_on_redeemed_by_id"
  end

  create_table "bug_reports", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.text "description", null: false
    t.integer "github_issue_number"
    t.string "github_issue_url"
    t.jsonb "metadata", default: {}, null: false
    t.string "page_url"
    t.integer "status", default: 0, null: false
    t.datetime "updated_at", null: false
    t.string "user_agent"
    t.uuid "user_id", null: false
    t.uuid "workspace_id", null: false
    t.index ["status"], name: "index_bug_reports_on_status"
    t.index ["user_id"], name: "index_bug_reports_on_user_id"
    t.index ["workspace_id", "created_at"], name: "index_bug_reports_on_workspace_id_and_created_at"
  end

  create_table "calendar_account_users", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.uuid "calendar_account_id", null: false
    t.boolean "can_manage", default: false, null: false
    t.boolean "can_read", default: true, null: false
    t.boolean "can_write", default: false, null: false
    t.datetime "created_at", null: false
    t.boolean "owner", default: false, null: false
    t.datetime "updated_at", null: false
    t.uuid "user_id", null: false
    t.index ["calendar_account_id", "user_id"], name: "index_calendar_account_users_on_account_and_user", unique: true
    t.index ["calendar_account_id"], name: "index_calendar_account_users_on_calendar_account_id"
    t.index ["user_id"], name: "index_calendar_account_users_on_user_id"
  end

  create_table "calendar_accounts", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.boolean "active", default: true, null: false
    t.string "color", default: "#3b82f6", null: false
    t.datetime "created_at", null: false
    t.string "email_address", null: false
    t.datetime "last_scanned_at"
    t.string "name"
    t.integer "provider", default: 0, null: false
    t.string "provider_account_id"
    t.string "refresh_token", null: false
    t.datetime "scan_started_at"
    t.boolean "scanning", default: false, null: false
    t.datetime "updated_at", null: false
    t.uuid "workspace_id", null: false
    t.index ["active"], name: "index_calendar_accounts_active", where: "(active = true)"
    t.index ["email_address", "provider"], name: "index_calendar_accounts_on_email_and_provider", unique: true
    t.index ["provider"], name: "index_calendar_accounts_on_provider"
    t.index ["workspace_id"], name: "index_calendar_accounts_on_workspace_id"
  end

  create_table "calendar_events", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.boolean "all_day", default: false, null: false
    t.jsonb "attendees", default: [], null: false
    t.uuid "calendar_id", null: false
    t.string "conference_url"
    t.datetime "created_at", null: false
    t.text "description"
    t.datetime "end_at"
    t.string "end_time_zone"
    t.uuid "event_type_id"
    t.string "html_link"
    t.string "ics_uid"
    t.boolean "is_organizer", default: false, null: false
    t.string "location"
    t.datetime "original_start_at"
    t.boolean "outbound_pending", default: false, null: false
    t.string "provider_etag"
    t.string "provider_event_id", null: false
    t.integer "provider_sequence"
    t.string "recurring_event_provider_id"
    t.string "rrule"
    t.integer "rsvp_status"
    t.uuid "source_email_message_id"
    t.datetime "start_at"
    t.string "start_time_zone"
    t.integer "status", default: 0, null: false
    t.string "title"
    t.integer "type_status", default: 0, null: false
    t.datetime "updated_at", null: false
    t.index ["calendar_id", "ics_uid"], name: "index_calendar_events_on_calendar_and_ics_uid", unique: true, where: "(ics_uid IS NOT NULL)"
    t.index ["calendar_id", "provider_event_id"], name: "index_calendar_events_on_calendar_and_provider_id", unique: true
    t.index ["calendar_id"], name: "index_calendar_events_on_calendar_id"
    t.index ["event_type_id"], name: "index_calendar_events_on_event_type_id"
    t.index ["recurring_event_provider_id"], name: "index_calendar_events_on_recurring_event_provider_id"
    t.index ["source_email_message_id"], name: "index_calendar_events_on_source_email_message_id"
    t.index ["start_at", "end_at"], name: "index_calendar_events_on_range"
    t.index ["start_at"], name: "index_calendar_events_on_start_at"
    t.index ["status"], name: "index_calendar_events_on_status"
  end

  create_table "calendar_sync_logs", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.uuid "calendar_account_id", null: false
    t.datetime "completed_at"
    t.datetime "created_at", null: false
    t.jsonb "error_messages", default: []
    t.integer "errors_count", default: 0
    t.integer "events_found", default: 0
    t.integer "events_upserted", default: 0
    t.datetime "started_at"
    t.integer "status", default: 0, null: false
    t.datetime "updated_at", null: false
    t.index ["calendar_account_id"], name: "index_calendar_sync_logs_on_calendar_account_id"
  end

  create_table "calendar_webhook_channels", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.uuid "calendar_id", null: false
    t.string "channel_token", null: false
    t.datetime "created_at", null: false
    t.datetime "expires_at"
    t.string "provider_channel_id", null: false
    t.string "provider_resource_id"
    t.datetime "updated_at", null: false
    t.index ["calendar_id"], name: "index_calendar_webhook_channels_on_calendar_id"
    t.index ["expires_at"], name: "index_calendar_webhook_channels_on_expires_at"
    t.index ["provider_channel_id"], name: "index_calendar_webhook_channels_on_provider_channel_id", unique: true
  end

  create_table "calendars", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.uuid "calendar_account_id", null: false
    t.string "color"
    t.datetime "created_at", null: false
    t.text "description"
    t.boolean "is_primary", default: false, null: false
    t.boolean "is_writable", default: false, null: false
    t.datetime "last_event_sync_at"
    t.string "name", null: false
    t.string "provider_calendar_id", null: false
    t.string "sync_token"
    t.datetime "sync_window_end"
    t.datetime "sync_window_start"
    t.boolean "syncing", default: false, null: false
    t.string "time_zone"
    t.datetime "updated_at", null: false
    t.index ["calendar_account_id", "provider_calendar_id"], name: "index_calendars_on_account_and_provider_id", unique: true
    t.index ["calendar_account_id"], name: "index_calendars_on_calendar_account_id"
    t.index ["syncing"], name: "index_calendars_syncing", where: "(syncing = true)"
  end

  create_table "connections", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.string "auth_header_name"
    t.text "auth_secret"
    t.string "auth_type", default: "none", null: false
    t.string "auth_username"
    t.string "base_url", null: false
    t.datetime "created_at", null: false
    t.string "name", null: false
    t.datetime "updated_at", null: false
    t.uuid "workspace_id", null: false
    t.index ["workspace_id", "name"], name: "index_connections_on_workspace_id_and_name"
    t.index ["workspace_id"], name: "index_connections_on_workspace_id"
  end

  create_table "contact_email_aliases", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.uuid "contact_id", null: false
    t.datetime "created_at", null: false
    t.string "email", null: false
    t.datetime "updated_at", null: false
    t.index ["contact_id"], name: "index_contact_email_aliases_on_contact_id"
    t.index ["email"], name: "index_contact_email_aliases_on_email", unique: true
  end

  create_table "contact_tags", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.float "confidence"
    t.uuid "contact_id", null: false
    t.datetime "created_at", null: false
    t.integer "source", default: 0, null: false
    t.uuid "tag_id", null: false
    t.datetime "updated_at", null: false
    t.index ["contact_id", "tag_id"], name: "index_contact_tags_on_contact_id_and_tag_id", unique: true
    t.index ["contact_id"], name: "index_contact_tags_on_contact_id"
    t.index ["tag_id"], name: "index_contact_tags_on_tag_id"
  end

  create_table "contacts", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "analyzed_at"
    t.datetime "auto_tagged_at"
    t.jsonb "communication_patterns", default: {}
    t.text "context_summary"
    t.datetime "created_at", null: false
    t.float "duplicate_confidence"
    t.uuid "duplicate_of_id"
    t.text "duplicate_reason"
    t.string "email", null: false
    t.uuid "email_account_id"
    t.integer "email_count", default: 0
    t.datetime "last_email_at"
    t.integer "list_status", default: 0, null: false
    t.string "name"
    t.string "organization"
    t.uuid "person_id"
    t.text "raw_analysis"
    t.string "relationship_type"
    t.datetime "starred_at"
    t.float "suggested_confidence"
    t.uuid "suggested_person_id"
    t.text "suggested_reason"
    t.datetime "updated_at", null: false
    t.uuid "workspace_id"
    t.index ["duplicate_of_id"], name: "index_contacts_on_duplicate_of_id"
    t.index ["email"], name: "index_contacts_on_email", unique: true
    t.index ["email_account_id", "email"], name: "index_contacts_on_email_account_id_and_email", unique: true
    t.index ["email_account_id"], name: "index_contacts_on_email_account_id"
    t.index ["last_email_at"], name: "index_contacts_on_last_email_at"
    t.index ["person_id"], name: "index_contacts_on_person_id"
    t.index ["relationship_type"], name: "index_contacts_on_relationship_type"
    t.index ["suggested_person_id"], name: "index_contacts_on_suggested_person_id"
    t.index ["workspace_id", "list_status"], name: "index_contacts_on_workspace_id_and_list_status"
    t.index ["workspace_id", "starred_at"], name: "index_contacts_on_workspace_and_starred", where: "(starred_at IS NOT NULL)"
    t.index ["workspace_id"], name: "index_contacts_on_workspace_id"
  end

  create_table "devices", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.string "app_version"
    t.datetime "created_at", null: false
    t.datetime "last_active_at"
    t.integer "platform", null: false
    t.string "token", null: false
    t.datetime "updated_at", null: false
    t.uuid "user_id", null: false
    t.index ["token"], name: "index_devices_on_token", unique: true
    t.index ["user_id", "platform"], name: "index_devices_on_user_id_and_platform"
    t.index ["user_id"], name: "index_devices_on_user_id"
  end

  create_table "digest_issues", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.boolean "ai_used", default: false, null: false
    t.jsonb "content", default: {}, null: false
    t.datetime "created_at", null: false
    t.datetime "email_sent_at"
    t.string "error_message"
    t.datetime "period_end", null: false
    t.datetime "period_start", null: false
    t.uuid "scheduled_digest_id", null: false
    t.integer "status", default: 0, null: false
    t.datetime "updated_at", null: false
    t.uuid "user_id", null: false
    t.uuid "workspace_id", null: false
    t.index ["scheduled_digest_id", "period_end"], name: "index_digest_issues_on_digest_and_period_end", unique: true
    t.index ["scheduled_digest_id"], name: "index_digest_issues_on_scheduled_digest_id"
    t.index ["user_id", "created_at"], name: "index_digest_issues_on_user_and_created_at"
  end

  create_table "document_drive_uploads", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.uuid "document_id", null: false
    t.string "drive_file_id"
    t.text "error_message"
    t.string "status", default: "pending", null: false
    t.datetime "updated_at", null: false
    t.datetime "uploaded_at"
    t.uuid "zoho_drive_account_id", null: false
    t.index ["document_id"], name: "index_document_drive_uploads_on_document_id"
    t.index ["status"], name: "index_document_drive_uploads_on_status"
    t.index ["zoho_drive_account_id"], name: "index_document_drive_uploads_on_zoho_drive_account_id"
  end

  create_table "document_email_messages", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.uuid "document_id", null: false
    t.uuid "email_message_id", null: false
    t.index ["document_id", "email_message_id"], name: "idx_document_email_messages_unique", unique: true
    t.index ["document_id"], name: "index_document_email_messages_on_document_id"
    t.index ["email_message_id"], name: "index_document_email_messages_on_email_message_id"
  end

  create_table "document_templates", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.jsonb "ai_provenance", default: {}, null: false
    t.integer "ai_status", default: 0, null: false
    t.datetime "created_at", null: false
    t.text "description"
    t.text "html_content", default: "", null: false
    t.string "name", null: false
    t.datetime "updated_at", null: false
    t.jsonb "variables_schema", default: [], null: false
    t.uuid "workspace_id", null: false
    t.index ["workspace_id", "name"], name: "index_document_templates_on_workspace_id_and_name"
    t.index ["workspace_id"], name: "index_document_templates_on_workspace_id"
  end

  create_table "document_types", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.boolean "auto_star", default: false, null: false
    t.string "category"
    t.string "color", null: false
    t.datetime "created_at", null: false
    t.jsonb "extraction_schema"
    t.string "name", null: false
    t.datetime "updated_at", null: false
    t.uuid "workspace_id"
    t.index ["workspace_id", "name"], name: "index_document_types_on_workspace_and_name", unique: true
    t.index ["workspace_id"], name: "index_document_types_on_workspace_id"
  end

  create_table "documents", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.string "account_number"
    t.float "ai_confidence_score"
    t.text "ai_error"
    t.jsonb "ai_extraction_data", default: {}
    t.integer "ai_processing_attempts", default: 0
    t.integer "ai_status", default: 0, null: false
    t.text "ai_summary"
    t.integer "amount_cents"
    t.string "bank_name"
    t.string "buyer_nif"
    t.string "canonical_filename"
    t.string "client_name"
    t.string "client_nif"
    t.integer "closing_balance_cents"
    t.boolean "company_vat_present"
    t.string "content_hash"
    t.datetime "created_at", null: false
    t.string "currency", default: "EUR"
    t.text "description"
    t.date "document_date"
    t.integer "document_type", default: 0, null: false
    t.uuid "document_type_id"
    t.date "due_date"
    t.uuid "email_account_id"
    t.string "email_message_id"
    t.integer "expense_category"
    t.string "google_drive_file_id"
    t.text "google_drive_push_error"
    t.integer "google_drive_push_status", default: 0, null: false
    t.datetime "google_drive_pushed_at"
    t.string "invoice_number"
    t.jsonb "metadata"
    t.integer "opening_balance_cents"
    t.string "payment_method"
    t.date "period_end"
    t.date "period_start"
    t.datetime "posted_to_thread_at"
    t.string "receipt_number"
    t.integer "review_status", default: 0, null: false
    t.datetime "reviewed_at"
    t.uuid "reviewed_by_id"
    t.string "sender_name"
    t.integer "source", default: 0, null: false
    t.boolean "starred", default: false, null: false
    t.integer "tax_amount_cents"
    t.decimal "tax_rate", precision: 5, scale: 2
    t.datetime "updated_at", null: false
    t.string "vendor_name"
    t.string "vendor_nif"
    t.datetime "viewed_at"
    t.uuid "workspace_id"
    t.index ["ai_status"], name: "index_documents_on_ai_status"
    t.index ["client_nif"], name: "index_documents_on_client_nif"
    t.index ["content_hash"], name: "index_documents_on_content_hash"
    t.index ["document_date"], name: "index_documents_on_document_date"
    t.index ["document_type"], name: "index_documents_on_document_type"
    t.index ["document_type_id"], name: "index_documents_on_document_type_id"
    t.index ["email_account_id"], name: "index_documents_on_email_account_id"
    t.index ["email_message_id"], name: "index_documents_on_email_message_id"
    t.index ["review_status"], name: "index_documents_on_review_status"
    t.index ["reviewed_by_id"], name: "index_documents_on_reviewed_by_id"
    t.index ["source"], name: "index_documents_on_source"
    t.index ["vendor_nif"], name: "index_documents_on_vendor_nif"
    t.index ["workspace_id", "ai_status"], name: "index_documents_on_workspace_id_and_ai_status"
    t.index ["workspace_id", "due_date"], name: "index_documents_on_workspace_and_due_date", where: "(due_date IS NOT NULL)"
    t.index ["workspace_id", "review_status", "ai_confidence_score"], name: "index_documents_on_workspace_review_confidence"
    t.index ["workspace_id", "review_status"], name: "index_documents_on_workspace_id_and_review_status"
    t.index ["workspace_id", "starred"], name: "index_documents_on_workspace_id_and_starred"
    t.index ["workspace_id"], name: "index_documents_on_workspace_id"
    t.index ["workspace_id"], name: "index_documents_on_workspace_unviewed", where: "(viewed_at IS NULL)"
  end

  create_table "draft_emails", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.jsonb "attachments_json", default: [], null: false
    t.text "bcc_address"
    t.text "body"
    t.text "cc_address"
    t.datetime "created_at", null: false
    t.uuid "email_account_id"
    t.uuid "in_reply_to_id"
    t.integer "mode", default: 0, null: false
    t.text "quoted_body"
    t.uuid "signature_id"
    t.string "subject"
    t.text "to_address"
    t.datetime "updated_at", null: false
    t.uuid "user_id", null: false
    t.uuid "workspace_id", null: false
    t.index ["email_account_id"], name: "index_draft_emails_on_email_account_id"
    t.index ["in_reply_to_id"], name: "index_draft_emails_on_in_reply_to_id"
    t.index ["signature_id"], name: "index_draft_emails_on_signature_id"
    t.index ["user_id", "updated_at"], name: "index_draft_emails_on_user_id_and_updated_at"
    t.index ["user_id"], name: "index_draft_emails_on_user_id"
    t.index ["workspace_id"], name: "index_draft_emails_on_workspace_id"
  end

  create_table "drive_folder_mappings", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.boolean "auto_sync", default: false, null: false
    t.datetime "created_at", null: false
    t.uuid "document_type_id"
    t.string "drive_folder_id", null: false
    t.string "drive_folder_path"
    t.datetime "updated_at", null: false
    t.uuid "zoho_drive_account_id", null: false
    t.index ["document_type_id"], name: "index_drive_folder_mappings_on_document_type_id"
    t.index ["zoho_drive_account_id", "document_type_id"], name: "idx_drive_folder_mappings_on_account_and_type", unique: true
    t.index ["zoho_drive_account_id"], name: "index_drive_folder_mappings_on_zoho_drive_account_id"
  end

  create_table "email_account_signatures", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.uuid "email_account_id", null: false
    t.uuid "signature_id", null: false
    t.datetime "updated_at", null: false
    t.index ["email_account_id"], name: "index_email_account_signatures_on_email_account_id"
    t.index ["signature_id", "email_account_id"], name: "idx_on_signature_id_email_account_id_7999730e85", unique: true
    t.index ["signature_id"], name: "index_email_account_signatures_on_signature_id"
  end

  create_table "email_account_users", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.boolean "can_manage", default: false, null: false
    t.boolean "can_read", default: true, null: false
    t.boolean "can_send", default: false, null: false
    t.datetime "created_at", null: false
    t.uuid "email_account_id", null: false
    t.boolean "owner", default: false, null: false
    t.datetime "updated_at", null: false
    t.uuid "user_id", null: false
    t.index ["email_account_id", "user_id"], name: "index_email_account_users_on_email_account_id_and_user_id", unique: true
    t.index ["email_account_id"], name: "index_email_account_users_on_email_account_id"
    t.index ["user_id"], name: "index_email_account_users_on_user_id"
  end

  create_table "email_accounts", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.boolean "active", default: true, null: false
    t.string "color", default: "#3b82f6", null: false
    t.datetime "created_at", null: false
    t.string "email_address", null: false
    t.string "history_id"
    t.datetime "last_scanned_at"
    t.string "name"
    t.integer "provider", default: 0, null: false
    t.string "provider_account_id"
    t.string "refresh_token", null: false
    t.datetime "scan_started_at"
    t.boolean "scanning", default: false, null: false
    t.datetime "updated_at", null: false
    t.uuid "workspace_id"
    t.index ["email_address"], name: "index_email_accounts_on_email_address", unique: true
    t.index ["provider"], name: "index_email_accounts_on_provider"
    t.index ["workspace_id"], name: "index_email_accounts_on_workspace_id"
  end

  create_table "email_folders", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.string "delta_token"
    t.uuid "email_account_id", null: false
    t.datetime "last_synced_at"
    t.string "name", null: false
    t.integer "position", default: 0, null: false
    t.string "provider_folder_id", null: false
    t.datetime "updated_at", null: false
    t.index ["email_account_id", "provider_folder_id"], name: "index_email_folders_on_email_account_id_and_provider_folder_id", unique: true
    t.index ["email_account_id"], name: "index_email_folders_on_email_account_id"
  end

  create_table "email_message_tags", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.uuid "email_message_id", null: false
    t.uuid "tag_id", null: false
    t.datetime "updated_at", null: false
    t.index ["email_message_id", "tag_id"], name: "idx_email_message_tags_unique", unique: true
    t.index ["email_message_id"], name: "index_email_message_tags_on_email_message_id"
    t.index ["tag_id"], name: "index_email_message_tags_on_tag_id"
  end

  create_table "email_messages", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.text "ai_action_prompt"
    t.uuid "ai_analysis_message_id"
    t.datetime "ai_analyzed_at"
    t.integer "ai_priority", default: 1, null: false
    t.jsonb "ai_provenance", default: {}, null: false
    t.jsonb "ai_suggested_actions", default: [], null: false
    t.text "ai_summary"
    t.boolean "ai_todo_dismissed", default: false, null: false
    t.text "bcc_address"
    t.text "body"
    t.datetime "categorized_at"
    t.string "category"
    t.float "category_confidence"
    t.text "cc_address"
    t.uuid "contact_id"
    t.datetime "created_at", null: false
    t.uuid "email_account_id", null: false
    t.uuid "email_scan_log_id"
    t.uuid "email_thread_id"
    t.string "from_address"
    t.boolean "has_attachment"
    t.string "header_auto_submitted", comment: "RFC 3834 Auto-Submitted (anything but 'no' => machine-generated)"
    t.text "header_list_unsubscribe", comment: "RFC 2369 List-Unsubscribe value; presence => list/bulk mail"
    t.string "header_precedence", comment: "RFC 2076 Precedence (bulk/list/junk => bulk mail)"
    t.datetime "pinned_at"
    t.string "provider_folder_id"
    t.jsonb "provider_labels", default: [], null: false
    t.string "provider_message_id", null: false
    t.string "provider_thread_id"
    t.boolean "read", default: false, null: false
    t.datetime "received_at"
    t.datetime "skimmed_at"
    t.integer "status", default: 0, null: false
    t.string "subject"
    t.text "summary"
    t.string "to_address"
    t.datetime "updated_at", null: false
    t.datetime "viewed_at"
    t.string "zoho_flag"
    t.index ["ai_analysis_message_id"], name: "index_email_messages_on_ai_analysis_message_id"
    t.index ["category"], name: "index_email_messages_on_category"
    t.index ["contact_id"], name: "index_email_messages_on_contact_id"
    t.index ["email_account_id", "provider_message_id"], name: "index_emails_on_account_and_provider_message", unique: true
    t.index ["email_account_id", "provider_thread_id"], name: "index_email_messages_on_account_and_provider_thread"
    t.index ["email_account_id"], name: "index_email_messages_on_account_unviewed", where: "(viewed_at IS NULL)"
    t.index ["email_account_id"], name: "index_email_messages_on_email_account_id"
    t.index ["email_scan_log_id"], name: "index_email_messages_on_email_scan_log_id"
    t.index ["email_thread_id"], name: "index_email_messages_on_email_thread_id"
    t.index ["pinned_at"], name: "index_email_messages_on_pinned_at"
    t.index ["read"], name: "index_email_messages_on_read"
    t.index ["received_at"], name: "idx_email_messages_ai_todos", order: :desc, where: "((ai_action_prompt IS NOT NULL) AND (ai_action_prompt <> ''::text) AND (ai_todo_dismissed = false))"
    t.index ["skimmed_at"], name: "index_email_messages_on_skimmed_at"
    t.index ["status"], name: "index_email_messages_on_status"
  end

  create_table "email_scan_logs", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "completed_at"
    t.datetime "created_at", null: false
    t.integer "documents_created", default: 0
    t.uuid "email_account_id", null: false
    t.integer "emails_found", default: 0
    t.integer "emails_processed", default: 0
    t.jsonb "error_messages", default: []
    t.integer "errors_count", default: 0
    t.datetime "started_at"
    t.integer "status", default: 0, null: false
    t.datetime "updated_at", null: false
    t.index ["email_account_id"], name: "index_email_scan_logs_on_email_account_id"
  end

  create_table "email_template_documents", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.uuid "document_template_id", null: false
    t.uuid "email_template_id", null: false
    t.integer "position", default: 0, null: false
    t.datetime "updated_at", null: false
    t.index ["document_template_id"], name: "index_email_template_documents_on_document_template_id"
    t.index ["email_template_id", "document_template_id"], name: "idx_email_template_documents_unique", unique: true
    t.index ["email_template_id"], name: "index_email_template_documents_on_email_template_id"
  end

  create_table "email_templates", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.jsonb "ai_provenance", default: {}, null: false
    t.integer "ai_status", default: 0, null: false
    t.text "body_html", default: "", null: false
    t.datetime "created_at", null: false
    t.text "description"
    t.string "name", null: false
    t.string "subject", default: "", null: false
    t.datetime "updated_at", null: false
    t.jsonb "variables_schema", default: [], null: false
    t.uuid "workspace_id", null: false
    t.index ["workspace_id", "name"], name: "index_email_templates_on_workspace_id_and_name"
    t.index ["workspace_id"], name: "index_email_templates_on_workspace_id"
  end

  create_table "email_threads", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.uuid "email_account_id", null: false
    t.datetime "follow_up_at"
    t.datetime "follow_up_dismissed_at"
    t.boolean "follow_up_expected", default: false, null: false
    t.datetime "follow_up_last_analyzed_at"
    t.uuid "follow_up_outbound_message_id"
    t.string "follow_up_reason"
    t.datetime "last_inbound_at"
    t.datetime "last_outbound_at"
    t.string "provider_thread_id"
    t.datetime "snoozed_until"
    t.string "subject", null: false
    t.string "subject_key"
    t.datetime "updated_at", null: false
    t.index ["email_account_id", "provider_thread_id"], name: "index_email_threads_on_account_and_provider_thread_uniq", unique: true, where: "(provider_thread_id IS NOT NULL)"
    t.index ["email_account_id", "subject_key"], name: "index_email_threads_on_account_and_subject_key"
    t.index ["email_account_id"], name: "index_email_threads_on_email_account_id"
    t.index ["follow_up_at"], name: "index_email_threads_on_due_follow_ups", where: "(follow_up_expected AND (follow_up_dismissed_at IS NULL))"
    t.index ["snoozed_until"], name: "index_email_threads_on_snoozed_until_not_null", where: "(snoozed_until IS NOT NULL)"
  end

  create_table "event_types", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.string "icon"
    t.string "name", null: false
    t.datetime "updated_at", null: false
    t.uuid "workspace_id", null: false
    t.index ["workspace_id", "name"], name: "index_event_types_on_workspace_and_name", unique: true
    t.index ["workspace_id"], name: "index_event_types_on_workspace_id"
  end

  create_table "events", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.uuid "actor_id"
    t.string "actor_type"
    t.uuid "caused_by_event_id"
    t.datetime "created_at", null: false
    t.integer "depth", default: 0, null: false
    t.string "name", null: false
    t.datetime "occurred_at", null: false
    t.jsonb "payload", default: {}, null: false
    t.uuid "subject_id"
    t.string "subject_type"
    t.datetime "updated_at", null: false
    t.uuid "workspace_id", null: false
    t.index ["actor_type", "actor_id"], name: "index_events_on_actor"
    t.index ["caused_by_event_id"], name: "index_events_on_caused_by_event_id"
    t.index ["subject_type", "subject_id"], name: "index_events_on_subject"
    t.index ["workspace_id", "name", "occurred_at"], name: "index_events_on_workspace_id_and_name_and_occurred_at"
    t.index ["workspace_id", "occurred_at"], name: "index_events_on_workspace_id_and_occurred_at"
    t.index ["workspace_id"], name: "index_events_on_workspace_id"
  end

  create_table "exports", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.integer "documents_count"
    t.jsonb "filters"
    t.integer "status"
    t.datetime "updated_at", null: false
    t.uuid "workspace_id", null: false
    t.index ["workspace_id"], name: "index_exports_on_workspace_id"
  end

  create_table "feed_items", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "acted_at"
    t.boolean "attention", default: false, null: false
    t.datetime "created_at", null: false
    t.jsonb "data", default: {}, null: false
    t.string "dedupe_key", null: false
    t.datetime "dismissed_at"
    t.datetime "expired_at"
    t.datetime "generated_at"
    t.string "kind", null: false
    t.integer "score", default: 0, null: false
    t.datetime "seen_at"
    t.datetime "sort_at", null: false
    t.uuid "subject_id", null: false
    t.string "subject_type", null: false
    t.datetime "updated_at", null: false
    t.uuid "user_id", null: false
    t.uuid "workspace_id", null: false
    t.index ["subject_type", "subject_id"], name: "idx_feed_items_subject"
    t.index ["user_id", "dedupe_key"], name: "idx_feed_items_user_dedupe", unique: true
    t.index ["user_id", "score", "sort_at"], name: "idx_feed_items_attention", order: { score: :desc, sort_at: :desc }, where: "((dismissed_at IS NULL) AND (acted_at IS NULL) AND (expired_at IS NULL) AND (attention = true))"
    t.index ["user_id", "score", "sort_at"], name: "idx_feed_items_timeline", order: { score: :desc, sort_at: :desc }, where: "((dismissed_at IS NULL) AND (acted_at IS NULL) AND (expired_at IS NULL) AND (attention = false))"
    t.index ["user_id"], name: "index_feed_items_on_user_unseen_active", where: "((seen_at IS NULL) AND (dismissed_at IS NULL) AND (acted_at IS NULL) AND (expired_at IS NULL))"
    t.index ["workspace_id"], name: "index_feed_items_on_workspace_id"
  end

  create_table "file_share_links", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.uuid "created_by_id"
    t.datetime "expires_at"
    t.datetime "last_viewed_at"
    t.datetime "revoked_at"
    t.uuid "shareable_id", null: false
    t.string "shareable_type", null: false
    t.string "token", null: false
    t.datetime "updated_at", null: false
    t.integer "view_count", default: 0, null: false
    t.uuid "workspace_id", null: false
    t.index ["created_by_id"], name: "index_file_share_links_on_created_by_id"
    t.index ["shareable_type", "shareable_id"], name: "index_file_share_links_on_shareable_type_and_shareable_id"
    t.index ["token"], name: "index_file_share_links_on_token", unique: true
    t.index ["workspace_id"], name: "index_file_share_links_on_workspace_id"
  end

  create_table "folder_memberships", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.uuid "folderable_id", null: false
    t.string "folderable_type", null: false
    t.uuid "mail_folder_id", null: false
    t.integer "position", default: 0, null: false
    t.datetime "updated_at", null: false
    t.index ["folderable_type", "folderable_id"], name: "index_folder_memberships_on_folderable"
    t.index ["mail_folder_id", "folderable_type", "folderable_id"], name: "index_folder_memberships_unique", unique: true
    t.index ["mail_folder_id"], name: "index_folder_memberships_on_mail_folder_id"
  end

  create_table "google_drive_accounts", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.boolean "connected", default: true, null: false
    t.datetime "created_at", null: false
    t.string "email"
    t.string "refresh_token", null: false
    t.string "scopes"
    t.datetime "updated_at", null: false
    t.uuid "workspace_id"
    t.index ["workspace_id"], name: "index_google_drive_accounts_on_workspace_id"
  end

  create_table "google_drive_configs", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.boolean "auto_push", default: false, null: false
    t.datetime "created_at", null: false
    t.uuid "document_type_id", null: false
    t.string "folder_id"
    t.string "folder_path"
    t.string "naming_pattern", default: "{date}_{entity}_{reference}", null: false
    t.integer "subfolder_pattern", default: 0, null: false
    t.datetime "updated_at", null: false
    t.index ["document_type_id"], name: "index_google_drive_configs_on_document_type_id", unique: true
  end

  create_table "identities", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.string "email"
    t.string "provider", null: false
    t.string "uid", null: false
    t.datetime "updated_at", null: false
    t.uuid "user_id", null: false
    t.index ["provider", "uid"], name: "index_identities_on_provider_and_uid", unique: true
    t.index ["user_id"], name: "index_identities_on_user_id"
  end

  create_table "invitations", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "accepted_at"
    t.uuid "accepted_by_id"
    t.boolean "admin_approved", default: true, null: false
    t.datetime "created_at", null: false
    t.string "email", null: false
    t.datetime "expires_at", null: false
    t.uuid "invited_by_id", null: false
    t.integer "status", default: 0, null: false
    t.string "token", null: false
    t.datetime "updated_at", null: false
    t.uuid "workspace_id", null: false
    t.index ["accepted_by_id"], name: "index_invitations_on_accepted_by_id"
    t.index ["admin_approved"], name: "index_invitations_on_admin_approved"
    t.index ["email", "workspace_id", "status"], name: "idx_invitations_on_email_workspace_status"
    t.index ["invited_by_id"], name: "index_invitations_on_invited_by_id"
    t.index ["token"], name: "index_invitations_on_token", unique: true
    t.index ["workspace_id"], name: "index_invitations_on_workspace_id"
  end

  create_table "learning_decisions", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.string "category"
    t.uuid "contact_id"
    t.datetime "created_at", null: false
    t.string "domain", null: false
    t.string "label", null: false
    t.string "sender_domain"
    t.jsonb "signals", default: {}, null: false
    t.uuid "subject_id"
    t.string "subject_type"
    t.datetime "updated_at", null: false
    t.uuid "user_id", null: false
    t.uuid "workspace_id", null: false
    t.index ["contact_id"], name: "index_learning_decisions_on_contact_id"
    t.index ["user_id", "domain", "created_at"], name: "index_learning_decisions_on_user_domain_time"
    t.index ["workspace_id", "domain", "created_at"], name: "index_learning_decisions_on_workspace_domain_time"
  end

  create_table "mail_folder_users", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.boolean "can_manage", default: false, null: false
    t.boolean "can_read", default: true, null: false
    t.boolean "can_write", default: false, null: false
    t.datetime "created_at", null: false
    t.uuid "mail_folder_id", null: false
    t.boolean "owner", default: false, null: false
    t.datetime "updated_at", null: false
    t.uuid "user_id", null: false
    t.index ["mail_folder_id", "user_id"], name: "index_mail_folder_users_on_mail_folder_id_and_user_id", unique: true
    t.index ["mail_folder_id"], name: "index_mail_folder_users_on_mail_folder_id"
    t.index ["user_id"], name: "index_mail_folder_users_on_user_id"
  end

  create_table "mail_folders", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.string "icon"
    t.string "name", null: false
    t.uuid "parent_id"
    t.integer "position", default: 0, null: false
    t.boolean "restricted", default: false, null: false
    t.datetime "updated_at", null: false
    t.uuid "workspace_id", null: false
    t.index "workspace_id, lower((name)::text)", name: "index_mail_folders_on_workspace_and_lower_name", unique: true
    t.index ["parent_id"], name: "index_mail_folders_on_parent_id"
    t.index ["workspace_id"], name: "index_mail_folders_on_workspace_id"
  end

  create_table "mfa_email_challenges", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.integer "attempts", default: 0, null: false
    t.string "code_digest", null: false
    t.datetime "created_at", null: false
    t.datetime "expires_at", null: false
    t.datetime "updated_at", null: false
    t.uuid "user_id", null: false
    t.index ["expires_at"], name: "index_mfa_email_challenges_on_expires_at"
    t.index ["user_id"], name: "index_mfa_email_challenges_on_user_id", unique: true
  end

  create_table "notification_preferences", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.uuid "document_type_id"
    t.boolean "enabled", default: true, null: false
    t.integer "kind", null: false
    t.boolean "notify_email", default: false, null: false
    t.boolean "notify_in_app", default: true, null: false
    t.uuid "tag_id"
    t.datetime "updated_at", null: false
    t.uuid "user_id", null: false
    t.index ["document_type_id"], name: "index_notification_preferences_on_document_type_id"
    t.index ["tag_id"], name: "index_notification_preferences_on_tag_id"
    t.index ["user_id", "kind", "document_type_id"], name: "idx_notification_prefs_user_kind_doctype", unique: true, where: "(document_type_id IS NOT NULL)"
    t.index ["user_id", "kind", "tag_id"], name: "idx_notification_prefs_user_kind_tag", unique: true, where: "(tag_id IS NOT NULL)"
    t.index ["user_id"], name: "index_notification_preferences_on_user_id"
  end

  create_table "notifications", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "archived_at"
    t.text "body"
    t.integer "category", default: 2, null: false
    t.integer "count", default: 1, null: false
    t.datetime "created_at", null: false
    t.string "group_key"
    t.string "link_url"
    t.uuid "notifiable_id"
    t.string "notifiable_type"
    t.integer "priority", default: 1, null: false
    t.boolean "read", default: false, null: false
    t.datetime "read_at"
    t.datetime "resolved_at"
    t.string "title", null: false
    t.datetime "updated_at", null: false
    t.uuid "user_id", null: false
    t.index ["archived_at"], name: "index_notifications_on_archived_at"
    t.index ["notifiable_type", "notifiable_id"], name: "index_notifications_on_notifiable"
    t.index ["resolved_at"], name: "index_notifications_on_resolved_at"
    t.index ["user_id", "category", "resolved_at", "archived_at"], name: "idx_notifications_active_by_category"
    t.index ["user_id", "group_key"], name: "index_notifications_on_user_id_and_group_key"
    t.index ["user_id", "read", "created_at"], name: "index_notifications_on_user_id_and_read_and_created_at"
    t.index ["user_id"], name: "index_notifications_on_user_id"
  end

  create_table "notion_database_mappings", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.uuid "document_type_id", null: false
    t.jsonb "field_mappings", default: {}
    t.string "notion_database_id", null: false
    t.string "notion_database_name"
    t.boolean "pull_enabled", default: false, null: false
    t.boolean "push_enabled", default: false, null: false
    t.datetime "updated_at", null: false
    t.index ["document_type_id"], name: "index_notion_database_mappings_on_document_type_id", unique: true
  end

  create_table "notion_integrations", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.text "access_token", null: false
    t.boolean "active", default: true, null: false
    t.uuid "authorized_by_user_id"
    t.string "bot_id"
    t.datetime "created_at", null: false
    t.string "notion_workspace_icon"
    t.string "notion_workspace_id"
    t.string "notion_workspace_name"
    t.datetime "updated_at", null: false
    t.uuid "workspace_id"
    t.index ["authorized_by_user_id"], name: "index_notion_integrations_on_authorized_by_user_id"
    t.index ["workspace_id", "notion_workspace_id"], name: "index_notion_integrations_on_workspace_and_notion_ws", unique: true
    t.index ["workspace_id"], name: "index_notion_integrations_on_workspace_id"
  end

  create_table "notion_pages", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.uuid "document_id", null: false
    t.text "last_error"
    t.datetime "last_synced_at"
    t.uuid "notion_database_mapping_id", null: false
    t.string "notion_page_id", null: false
    t.integer "sync_status", default: 0, null: false
    t.datetime "updated_at", null: false
    t.index ["document_id"], name: "index_notion_pages_on_document_id", unique: true
    t.index ["notion_database_mapping_id"], name: "index_notion_pages_on_notion_database_mapping_id"
    t.index ["notion_page_id"], name: "index_notion_pages_on_notion_page_id", unique: true
    t.index ["sync_status"], name: "index_notion_pages_on_sync_status"
  end

  create_table "oauth_access_grants", force: :cascade do |t|
    t.uuid "application_id", null: false
    t.string "code_challenge"
    t.string "code_challenge_method"
    t.datetime "created_at", null: false
    t.integer "expires_in", null: false
    t.text "redirect_uri"
    t.uuid "resource_owner_id", null: false
    t.datetime "revoked_at"
    t.string "scopes", default: "", null: false
    t.string "token", null: false
    t.index ["application_id"], name: "index_oauth_access_grants_on_application_id"
    t.index ["resource_owner_id"], name: "index_oauth_access_grants_on_resource_owner_id"
    t.index ["token"], name: "index_oauth_access_grants_on_token", unique: true
  end

  create_table "oauth_access_tokens", force: :cascade do |t|
    t.uuid "application_id", null: false
    t.datetime "created_at", null: false
    t.integer "expires_in"
    t.string "previous_refresh_token", default: "", null: false
    t.string "refresh_token"
    t.uuid "resource_owner_id"
    t.datetime "revoked_at"
    t.string "scopes"
    t.string "token", null: false
    t.index ["application_id"], name: "index_oauth_access_tokens_on_application_id"
    t.index ["refresh_token"], name: "index_oauth_access_tokens_on_refresh_token", unique: true
    t.index ["resource_owner_id"], name: "index_oauth_access_tokens_on_resource_owner_id"
    t.index ["token"], name: "index_oauth_access_tokens_on_token", unique: true
  end

  create_table "oauth_applications", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.boolean "confidential", default: true, null: false
    t.datetime "created_at", null: false
    t.uuid "created_by_id"
    t.string "name", null: false
    t.text "redirect_uri"
    t.string "scopes", default: "", null: false
    t.string "secret", null: false
    t.string "uid", null: false
    t.datetime "updated_at", null: false
    t.uuid "workspace_id"
    t.index ["created_by_id"], name: "index_oauth_applications_on_created_by_id"
    t.index ["uid"], name: "index_oauth_applications_on_uid", unique: true
    t.index ["workspace_id"], name: "index_oauth_applications_on_workspace_id"
  end

  create_table "organization_memberships", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.uuid "organization_id", null: false
    t.uuid "person_id", null: false
    t.integer "status", default: 0, null: false
    t.datetime "updated_at", null: false
    t.index ["organization_id", "status"], name: "index_organization_memberships_on_organization_id_and_status"
    t.index ["organization_id"], name: "index_organization_memberships_on_organization_id"
    t.index ["person_id", "organization_id"], name: "idx_on_person_id_organization_id_a4053ecbba", unique: true
    t.index ["person_id"], name: "index_organization_memberships_on_person_id"
  end

  create_table "organizations", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.string "domain"
    t.string "name", null: false
    t.text "notes"
    t.datetime "updated_at", null: false
    t.uuid "workspace_id", null: false
    t.index ["workspace_id", "name"], name: "index_organizations_on_workspace_id_and_name", unique: true
    t.index ["workspace_id"], name: "index_organizations_on_workspace_id"
  end

  create_table "people", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "analyzed_at"
    t.jsonb "communication_patterns", default: {}
    t.text "context_summary"
    t.datetime "created_at", null: false
    t.string "name"
    t.string "organization"
    t.text "raw_analysis"
    t.string "relationship_type"
    t.datetime "updated_at", null: false
    t.uuid "workspace_id"
    t.index ["name"], name: "index_people_on_name"
    t.index ["organization"], name: "index_people_on_organization"
    t.index ["relationship_type"], name: "index_people_on_relationship_type"
    t.index ["workspace_id"], name: "index_people_on_workspace_id"
  end

  create_table "pipeline_memberships", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.uuid "current_stage_id"
    t.datetime "entered_at"
    t.uuid "item_id", null: false
    t.string "item_type", null: false
    t.datetime "last_moved_at"
    t.uuid "pipeline_id", null: false
    t.integer "position", default: 0, null: false
    t.jsonb "stage_history", default: [], null: false
    t.datetime "updated_at", null: false
    t.index ["current_stage_id"], name: "index_pipeline_memberships_on_current_stage_id"
    t.index ["item_type", "item_id"], name: "idx_plm_on_item"
    t.index ["pipeline_id", "item_type", "item_id"], name: "idx_plm_on_pipeline_and_item", unique: true
    t.index ["pipeline_id"], name: "index_pipeline_memberships_on_pipeline_id"
  end

  create_table "pipeline_stages", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.string "color", default: "#6366f1", null: false
    t.datetime "created_at", null: false
    t.text "description"
    t.boolean "is_terminal", default: false, null: false
    t.string "name", null: false
    t.uuid "pipeline_id", null: false
    t.integer "position", null: false
    t.datetime "updated_at", null: false
    t.index ["pipeline_id", "name"], name: "index_pipeline_stages_on_pipeline_id_and_name", unique: true
    t.index ["pipeline_id", "position"], name: "index_pipeline_stages_on_pipeline_id_and_position"
    t.index ["pipeline_id"], name: "index_pipeline_stages_on_pipeline_id"
  end

  create_table "pipelines", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.integer "applies_to", default: 0, null: false
    t.datetime "created_at", null: false
    t.text "description"
    t.string "icon", default: "git-branch", null: false
    t.string "name", null: false
    t.integer "position", default: 0, null: false
    t.datetime "updated_at", null: false
    t.uuid "workspace_id", null: false
    t.index ["workspace_id", "name"], name: "index_pipelines_on_workspace_id_and_name", unique: true
    t.index ["workspace_id", "position"], name: "index_pipelines_on_workspace_id_and_position"
    t.index ["workspace_id"], name: "index_pipelines_on_workspace_id"
  end

  create_table "recovery_codes", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.string "code_digest", null: false
    t.datetime "created_at", null: false
    t.datetime "updated_at", null: false
    t.datetime "used_at"
    t.uuid "user_id", null: false
    t.index ["user_id"], name: "index_recovery_codes_on_user_id"
  end

  create_table "reminders", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.boolean "all_day", default: false, null: false
    t.integer "amount_cents"
    t.uuid "calendar_event_id"
    t.float "confidence", default: 0.0, null: false
    t.uuid "confirmed_by_id"
    t.datetime "created_at", null: false
    t.string "currency"
    t.text "description"
    t.datetime "due_at", null: false
    t.jsonb "extracted_data", default: {}, null: false
    t.string "extraction_fingerprint"
    t.text "justification"
    t.integer "reminder_type", null: false
    t.datetime "snoozed_until"
    t.uuid "source_id", null: false
    t.string "source_type", null: false
    t.integer "status", default: 0, null: false
    t.string "title", null: false
    t.datetime "updated_at", null: false
    t.datetime "viewed_at"
    t.uuid "workspace_id", null: false
    t.index ["calendar_event_id"], name: "index_reminders_on_calendar_event_id"
    t.index ["confirmed_by_id"], name: "index_reminders_on_confirmed_by_id"
    t.index ["extraction_fingerprint"], name: "index_reminders_on_fingerprint", unique: true, where: "(extraction_fingerprint IS NOT NULL)"
    t.index ["source_type", "source_id"], name: "index_reminders_on_source"
    t.index ["workspace_id", "status", "due_at"], name: "index_reminders_on_workspace_status_due"
    t.index ["workspace_id"], name: "index_reminders_on_workspace_id"
    t.index ["workspace_id"], name: "index_reminders_on_workspace_unviewed", where: "(viewed_at IS NULL)"
  end

  create_table "scheduled_digests", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.boolean "ai_enabled", default: true, null: false
    t.text "ai_instructions"
    t.jsonb "config", default: {}, null: false
    t.datetime "created_at", null: false
    t.boolean "deliver_by_email", default: true, null: false
    t.boolean "enabled", default: true, null: false
    t.datetime "last_run_at"
    t.string "name", null: false
    t.datetime "next_run_at", null: false
    t.string "preset_key"
    t.string "rrule", null: false
    t.boolean "show_in_feed", default: true, null: false
    t.datetime "updated_at", null: false
    t.uuid "user_id", null: false
    t.uuid "workspace_id", null: false
    t.index ["next_run_at"], name: "index_scheduled_digests_on_next_run_at_enabled", where: "enabled"
    t.index ["user_id"], name: "index_scheduled_digests_on_user_id"
    t.index ["workspace_id"], name: "index_scheduled_digests_on_workspace_id"
  end

  create_table "scheduled_emails", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.string "bcc_address"
    t.text "body", null: false
    t.string "cc_address"
    t.datetime "created_at", null: false
    t.uuid "created_by_id", null: false
    t.uuid "email_account_id", null: false
    t.uuid "email_template_id"
    t.datetime "last_sent_at"
    t.datetime "next_occurrence_at"
    t.string "rrule"
    t.datetime "scheduled_at", null: false
    t.integer "status", default: 0, null: false
    t.string "subject", null: false
    t.jsonb "template_context", default: {}, null: false
    t.string "to_address", null: false
    t.datetime "updated_at", null: false
    t.uuid "workspace_id", null: false
    t.index ["created_by_id"], name: "index_scheduled_emails_on_created_by_id"
    t.index ["email_account_id"], name: "index_scheduled_emails_on_email_account_id"
    t.index ["email_template_id"], name: "index_scheduled_emails_on_email_template_id"
    t.index ["next_occurrence_at"], name: "idx_scheduled_emails_pending_next_occurrence", where: "(status = 0)"
    t.index ["scheduled_at"], name: "idx_scheduled_emails_pending_scheduled_at", where: "(status = 0)"
    t.index ["workspace_id", "status"], name: "index_scheduled_emails_on_workspace_id_and_status"
    t.index ["workspace_id"], name: "index_scheduled_emails_on_workspace_id"
  end

  create_table "search_chunks", force: :cascade do |t|
    t.string "chunk_type", default: "text", null: false
    t.text "content", null: false
    t.datetime "created_at", null: false
    t.vector "embedding", limit: 1536
    t.string "embedding_model"
    t.jsonb "metadata", default: {}, null: false
    t.integer "position", default: 0, null: false
    t.uuid "searchable_id", null: false
    t.string "searchable_type", null: false
    t.integer "token_count"
    t.datetime "updated_at", null: false
    t.uuid "workspace_id", null: false
    t.index ["embedding"], name: "idx_search_chunks_embedding_hnsw", opclass: :vector_cosine_ops, using: :hnsw
    t.index ["metadata"], name: "index_search_chunks_on_metadata", using: :gin
    t.index ["searchable_type", "searchable_id"], name: "index_search_chunks_on_searchable_type_and_searchable_id"
    t.index ["workspace_id"], name: "index_search_chunks_on_workspace_id"
  end

  create_table "search_records", force: :cascade do |t|
    t.vector "content_embedding", limit: 1536
    t.text "content_preview"
    t.datetime "created_at", null: false
    t.string "embedding_model"
    t.jsonb "filter_data", default: {}, null: false
    t.datetime "indexed_at"
    t.uuid "searchable_id", null: false
    t.string "searchable_type", null: false
    t.datetime "source_created_at"
    t.datetime "source_updated_at"
    t.text "tags", default: [], array: true
    t.text "title"
    t.vector "title_embedding", limit: 1536
    t.datetime "updated_at", null: false
    t.uuid "workspace_id", null: false
    t.index ["content_embedding"], name: "idx_search_records_content_hnsw", opclass: :vector_cosine_ops, using: :hnsw
    t.index ["filter_data"], name: "index_search_records_on_filter_data", using: :gin
    t.index ["searchable_type", "searchable_id"], name: "index_search_records_on_searchable_type_and_searchable_id", unique: true
    t.index ["source_created_at"], name: "index_search_records_on_source_created_at"
    t.index ["tags"], name: "index_search_records_on_tags", using: :gin
    t.index ["title_embedding"], name: "idx_search_records_title_hnsw", opclass: :vector_cosine_ops, using: :hnsw
    t.index ["workspace_id"], name: "index_search_records_on_workspace_id"
  end

  create_table "search_tag_embeddings", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.string "content_hash"
    t.datetime "created_at", null: false
    t.vector "embedding", limit: 1536, null: false
    t.string "embedding_model"
    t.uuid "tag_id", null: false
    t.datetime "updated_at", null: false
    t.uuid "workspace_id", null: false
    t.index ["embedding"], name: "idx_search_tag_embeddings_hnsw", opclass: :vector_cosine_ops, using: :hnsw
    t.index ["tag_id"], name: "index_search_tag_embeddings_on_tag_id", unique: true
    t.index ["workspace_id"], name: "index_search_tag_embeddings_on_workspace_id"
  end

  create_table "sessions", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.string "ip_address"
    t.datetime "updated_at", null: false
    t.string "user_agent"
    t.uuid "user_id", null: false
    t.index ["user_id"], name: "index_sessions_on_user_id"
  end

  create_table "signatures", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.text "content", null: false
    t.datetime "created_at", null: false
    t.boolean "is_default", default: false, null: false
    t.string "name", null: false
    t.datetime "updated_at", null: false
    t.uuid "user_id", null: false
    t.index ["user_id", "is_default"], name: "index_signatures_on_user_id_and_is_default"
    t.index ["user_id", "name"], name: "index_signatures_on_user_id_and_name", unique: true
    t.index ["user_id"], name: "index_signatures_on_user_id"
  end

  create_table "signup_requests", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.uuid "accepted_by_id"
    t.datetime "created_at", null: false
    t.string "email", null: false
    t.string "name", null: false
    t.datetime "reviewed_at"
    t.uuid "reviewed_by_id"
    t.integer "status", default: 0, null: false
    t.string "token", null: false
    t.datetime "updated_at", null: false
    t.index ["accepted_by_id"], name: "index_signup_requests_on_accepted_by_id"
    t.index ["email", "status"], name: "index_signup_requests_on_email_and_status"
    t.index ["reviewed_by_id"], name: "index_signup_requests_on_reviewed_by_id"
    t.index ["token"], name: "index_signup_requests_on_token", unique: true
  end

  create_table "skim_decisions", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.string "action", null: false
    t.string "category"
    t.uuid "contact_id"
    t.datetime "created_at", null: false
    t.uuid "email_message_id"
    t.string "sender_domain"
    t.datetime "updated_at", null: false
    t.uuid "user_id", null: false
    t.uuid "workspace_id", null: false
    t.index ["contact_id"], name: "index_skim_decisions_on_contact_id"
    t.index ["user_id", "created_at"], name: "index_skim_decisions_on_user_id_and_created_at"
    t.index ["user_id"], name: "index_skim_decisions_on_user_id"
    t.index ["workspace_id"], name: "index_skim_decisions_on_workspace_id"
  end

  create_table "solid_cable_messages", force: :cascade do |t|
    t.binary "channel", null: false
    t.bigint "channel_hash", null: false
    t.datetime "created_at", null: false
    t.binary "payload", null: false
    t.index ["channel"], name: "index_solid_cable_messages_on_channel"
    t.index ["channel_hash"], name: "index_solid_cable_messages_on_channel_hash"
    t.index ["created_at"], name: "index_solid_cable_messages_on_created_at"
  end

  create_table "solid_queue_blocked_executions", force: :cascade do |t|
    t.string "concurrency_key", null: false
    t.datetime "created_at", null: false
    t.datetime "expires_at", null: false
    t.bigint "job_id", null: false
    t.integer "priority", default: 0, null: false
    t.string "queue_name", null: false
    t.index ["concurrency_key", "priority", "job_id"], name: "index_solid_queue_blocked_executions_for_release"
    t.index ["expires_at", "concurrency_key"], name: "index_solid_queue_blocked_executions_for_maintenance"
    t.index ["job_id"], name: "index_solid_queue_blocked_executions_on_job_id", unique: true
  end

  create_table "solid_queue_claimed_executions", force: :cascade do |t|
    t.datetime "created_at", null: false
    t.bigint "job_id", null: false
    t.bigint "process_id"
    t.index ["job_id"], name: "index_solid_queue_claimed_executions_on_job_id", unique: true
    t.index ["process_id", "job_id"], name: "index_solid_queue_claimed_executions_on_process_id_and_job_id"
  end

  create_table "solid_queue_failed_executions", force: :cascade do |t|
    t.datetime "created_at", null: false
    t.text "error"
    t.bigint "job_id", null: false
    t.index ["job_id"], name: "index_solid_queue_failed_executions_on_job_id", unique: true
  end

  create_table "solid_queue_jobs", force: :cascade do |t|
    t.string "active_job_id"
    t.text "arguments"
    t.string "class_name", null: false
    t.string "concurrency_key"
    t.datetime "created_at", null: false
    t.datetime "finished_at"
    t.integer "priority", default: 0, null: false
    t.string "queue_name", null: false
    t.datetime "scheduled_at"
    t.datetime "updated_at", null: false
    t.index ["active_job_id"], name: "index_solid_queue_jobs_on_active_job_id"
    t.index ["class_name"], name: "index_solid_queue_jobs_on_class_name"
    t.index ["finished_at"], name: "index_solid_queue_jobs_on_finished_at"
    t.index ["queue_name", "finished_at"], name: "index_solid_queue_jobs_for_filtering"
    t.index ["scheduled_at", "finished_at"], name: "index_solid_queue_jobs_for_alerting"
  end

  create_table "solid_queue_pauses", force: :cascade do |t|
    t.datetime "created_at", null: false
    t.string "queue_name", null: false
    t.index ["queue_name"], name: "index_solid_queue_pauses_on_queue_name", unique: true
  end

  create_table "solid_queue_processes", force: :cascade do |t|
    t.datetime "created_at", null: false
    t.string "hostname"
    t.string "kind", null: false
    t.datetime "last_heartbeat_at", null: false
    t.text "metadata"
    t.string "name", null: false
    t.integer "pid", null: false
    t.bigint "supervisor_id"
    t.index ["last_heartbeat_at"], name: "index_solid_queue_processes_on_last_heartbeat_at"
    t.index ["name", "supervisor_id"], name: "index_solid_queue_processes_on_name_and_supervisor_id", unique: true
    t.index ["supervisor_id"], name: "index_solid_queue_processes_on_supervisor_id"
  end

  create_table "solid_queue_ready_executions", force: :cascade do |t|
    t.datetime "created_at", null: false
    t.bigint "job_id", null: false
    t.integer "priority", default: 0, null: false
    t.string "queue_name", null: false
    t.index ["job_id"], name: "index_solid_queue_ready_executions_on_job_id", unique: true
    t.index ["priority", "job_id"], name: "index_solid_queue_poll_all"
    t.index ["queue_name", "priority", "job_id"], name: "index_solid_queue_poll_by_queue"
  end

  create_table "solid_queue_recurring_executions", force: :cascade do |t|
    t.datetime "created_at", null: false
    t.bigint "job_id", null: false
    t.datetime "run_at", null: false
    t.string "task_key", null: false
    t.index ["job_id"], name: "index_solid_queue_recurring_executions_on_job_id", unique: true
    t.index ["task_key", "run_at"], name: "index_solid_queue_recurring_executions_on_task_key_and_run_at", unique: true
  end

  create_table "solid_queue_recurring_tasks", force: :cascade do |t|
    t.text "arguments"
    t.string "class_name"
    t.string "command", limit: 2048
    t.datetime "created_at", null: false
    t.text "description"
    t.string "key", null: false
    t.integer "priority", default: 0
    t.string "queue_name"
    t.string "schedule", null: false
    t.boolean "static", default: true, null: false
    t.datetime "updated_at", null: false
    t.index ["key"], name: "index_solid_queue_recurring_tasks_on_key", unique: true
    t.index ["static"], name: "index_solid_queue_recurring_tasks_on_static"
  end

  create_table "solid_queue_scheduled_executions", force: :cascade do |t|
    t.datetime "created_at", null: false
    t.bigint "job_id", null: false
    t.integer "priority", default: 0, null: false
    t.string "queue_name", null: false
    t.datetime "scheduled_at", null: false
    t.index ["job_id"], name: "index_solid_queue_scheduled_executions_on_job_id", unique: true
    t.index ["scheduled_at", "priority", "job_id"], name: "index_solid_queue_dispatch_all"
  end

  create_table "solid_queue_semaphores", force: :cascade do |t|
    t.datetime "created_at", null: false
    t.datetime "expires_at", null: false
    t.string "key", null: false
    t.datetime "updated_at", null: false
    t.integer "value", default: 1, null: false
    t.index ["expires_at"], name: "index_solid_queue_semaphores_on_expires_at"
    t.index ["key", "value"], name: "index_solid_queue_semaphores_on_key_and_value"
    t.index ["key"], name: "index_solid_queue_semaphores_on_key", unique: true
  end

  create_table "tags", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.float "classification_confidence"
    t.string "classification_reason", limit: 255
    t.datetime "classified_at"
    t.string "color"
    t.datetime "created_at", null: false
    t.uuid "email_account_id"
    t.string "external_label_id"
    t.string "group_name"
    t.boolean "hidden", default: false, null: false
    t.integer "kind", default: 0, null: false
    t.string "name"
    t.integer "source", default: 0, null: false
    t.boolean "system_label", default: false, null: false
    t.datetime "updated_at", null: false
    t.uuid "workspace_id"
    t.index ["classified_at"], name: "index_tags_on_unclassified", where: "(classified_at IS NULL)"
    t.index ["email_account_id", "external_label_id"], name: "idx_tags_on_account_and_external_label_id", unique: true, where: "(external_label_id IS NOT NULL)"
    t.index ["email_account_id", "name"], name: "idx_tags_on_account_and_name", unique: true, where: "(email_account_id IS NOT NULL)"
    t.index ["email_account_id"], name: "index_tags_on_email_account_id"
    t.index ["external_label_id"], name: "index_tags_on_external_label_id"
    t.index ["hidden"], name: "index_tags_on_hidden", where: "(hidden = true)"
    t.index ["system_label"], name: "index_tags_on_system_label", where: "(system_label = true)"
    t.index ["workspace_id", "group_name"], name: "index_tags_on_workspace_id_and_group_name"
    t.index ["workspace_id"], name: "index_tags_on_workspace_id"
  end

  create_table "task_assignments", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.uuid "assigned_by_id"
    t.datetime "created_at", null: false
    t.uuid "task_id", null: false
    t.datetime "updated_at", null: false
    t.uuid "user_id", null: false
    t.index ["assigned_by_id"], name: "index_task_assignments_on_assigned_by_id"
    t.index ["task_id", "user_id"], name: "index_task_assignments_on_task_id_and_user_id", unique: true
    t.index ["task_id"], name: "index_task_assignments_on_task_id"
    t.index ["user_id"], name: "index_task_assignments_on_user_id"
  end

  create_table "task_documents", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.uuid "created_by_id"
    t.uuid "document_id", null: false
    t.uuid "task_id", null: false
    t.datetime "updated_at", null: false
    t.index ["created_by_id"], name: "index_task_documents_on_created_by_id"
    t.index ["document_id"], name: "index_task_documents_on_document_id"
    t.index ["task_id", "document_id"], name: "index_task_documents_on_task_id_and_document_id", unique: true
    t.index ["task_id"], name: "index_task_documents_on_task_id"
  end

  create_table "task_email_links", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.uuid "created_by_id"
    t.uuid "email_message_id", null: false
    t.integer "relationship", default: 0, null: false
    t.uuid "task_id", null: false
    t.datetime "updated_at", null: false
    t.index ["created_by_id"], name: "index_task_email_links_on_created_by_id"
    t.index ["email_message_id"], name: "index_task_email_links_on_email_message_id"
    t.index ["task_id", "email_message_id"], name: "index_task_email_links_on_task_id_and_email_message_id", unique: true
    t.index ["task_id"], name: "index_task_email_links_on_task_id"
  end

  create_table "task_tags", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.uuid "tag_id", null: false
    t.uuid "task_id", null: false
    t.datetime "updated_at", null: false
    t.index ["tag_id"], name: "index_task_tags_on_tag_id"
    t.index ["task_id", "tag_id"], name: "index_task_tags_on_task_id_and_tag_id", unique: true
    t.index ["task_id"], name: "index_task_tags_on_task_id"
  end

  create_table "tasks", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.boolean "ai_suggested", default: false, null: false
    t.boolean "all_day", default: false, null: false
    t.datetime "archived_at"
    t.datetime "completed_at"
    t.float "confidence", default: 0.0, null: false
    t.datetime "created_at", null: false
    t.uuid "created_by_id"
    t.text "description"
    t.datetime "due_at"
    t.jsonb "extracted_data", default: {}, null: false
    t.string "extraction_fingerprint"
    t.text "justification"
    t.integer "position", default: 0, null: false
    t.integer "priority", default: 1, null: false
    t.uuid "recurrence_parent_id"
    t.string "rrule"
    t.uuid "source_id"
    t.string "source_type"
    t.integer "status", default: 0, null: false
    t.string "title", null: false
    t.datetime "updated_at", null: false
    t.uuid "workspace_id", null: false
    t.index ["created_by_id"], name: "index_tasks_on_created_by_id"
    t.index ["extraction_fingerprint"], name: "index_tasks_on_fingerprint", unique: true, where: "(extraction_fingerprint IS NOT NULL)"
    t.index ["recurrence_parent_id"], name: "index_tasks_on_recurrence_parent_id"
    t.index ["source_type", "source_id"], name: "index_tasks_on_source"
    t.index ["workspace_id", "archived_at"], name: "index_tasks_on_workspace_id_and_archived_at"
    t.index ["workspace_id", "status", "due_at"], name: "index_tasks_on_workspace_status_due"
    t.index ["workspace_id"], name: "index_tasks_on_workspace_id"
  end

  create_table "templates", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.jsonb "data", default: {}, null: false
    t.text "description"
    t.string "name", null: false
    t.datetime "updated_at", null: false
    t.index ["name"], name: "index_templates_on_name", unique: true
  end

  create_table "thread_follows", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.uuid "agent_thread_id", null: false
    t.datetime "created_at", null: false
    t.datetime "updated_at", null: false
    t.uuid "user_id", null: false
    t.index ["agent_thread_id"], name: "index_thread_follows_on_agent_thread_id"
    t.index ["user_id", "agent_thread_id"], name: "index_thread_follows_on_user_id_and_agent_thread_id", unique: true
  end

  create_table "users", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.integer "compose_default", default: 0, null: false
    t.datetime "created_at", null: false
    t.datetime "deletion_requested_at"
    t.jsonb "dismissed_tours", default: [], null: false
    t.string "email_address", null: false
    t.boolean "email_on_mention", default: true, null: false
    t.boolean "email_on_thread_activity", default: true, null: false
    t.boolean "email_on_waiting_on_replies_digest", default: true, null: false
    t.datetime "email_otp_enabled_at"
    t.jsonb "hidden_calendar_ids", default: [], null: false
    t.jsonb "inbox_smart_groups", default: {}, null: false
    t.string "locale"
    t.datetime "mfa_last_totp_at"
    t.string "name"
    t.string "password_digest", null: false
    t.boolean "password_set_by_user", default: false, null: false
    t.integer "role", default: 0, null: false
    t.jsonb "section_seen_at", default: {}, null: false
    t.datetime "terms_accepted_at"
    t.datetime "totp_enabled_at"
    t.text "totp_secret"
    t.datetime "updated_at", null: false
    t.string "webauthn_id"
    t.uuid "workspace_id"
    t.text "writing_style"
    t.text "writing_style_learned"
    t.datetime "writing_style_updated_at"
    t.string "zoho_uid"
    t.index ["email_address"], name: "index_users_on_email_address", unique: true
    t.index ["role"], name: "index_users_on_role"
    t.index ["webauthn_id"], name: "index_users_on_webauthn_id", unique: true, where: "(webauthn_id IS NOT NULL)"
    t.index ["workspace_id"], name: "index_users_on_workspace_id"
  end

  create_table "webauthn_credentials", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.string "external_id", null: false
    t.datetime "last_used_at"
    t.string "nickname"
    t.text "public_key", null: false
    t.integer "sign_count", default: 0, null: false
    t.datetime "updated_at", null: false
    t.uuid "user_id", null: false
    t.index ["external_id"], name: "index_webauthn_credentials_on_external_id", unique: true
    t.index ["user_id"], name: "index_webauthn_credentials_on_user_id"
  end

  create_table "workflow_execution_steps", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "completed_at"
    t.datetime "created_at", null: false
    t.text "error_message"
    t.jsonb "input_data", default: {}
    t.jsonb "output_data", default: {}
    t.datetime "started_at"
    t.integer "status", default: 0, null: false
    t.datetime "updated_at", null: false
    t.uuid "workflow_execution_id", null: false
    t.uuid "workflow_step_id", null: false
    t.index ["workflow_execution_id"], name: "index_workflow_execution_steps_on_workflow_execution_id"
    t.index ["workflow_step_id"], name: "index_workflow_execution_steps_on_workflow_step_id"
  end

  create_table "workflow_executions", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "completed_at"
    t.datetime "created_at", null: false
    t.text "error_message"
    t.datetime "started_at"
    t.integer "status", default: 0, null: false
    t.jsonb "trigger_data", default: {}
    t.datetime "updated_at", null: false
    t.uuid "workflow_id", null: false
    t.uuid "workspace_id", null: false
    t.index ["workflow_id"], name: "index_workflow_executions_on_workflow_id"
    t.index ["workspace_id"], name: "index_workflow_executions_on_workspace_id"
  end

  create_table "workflow_steps", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.string "action_type"
    t.jsonb "config", default: {}
    t.datetime "created_at", null: false
    t.integer "position", null: false
    t.string "step_type", null: false
    t.datetime "updated_at", null: false
    t.uuid "workflow_id", null: false
    t.index ["workflow_id", "position"], name: "index_workflow_steps_on_workflow_id_and_position"
    t.index ["workflow_id"], name: "index_workflow_steps_on_workflow_id"
  end

  create_table "workflows", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.uuid "created_by_id"
    t.text "description"
    t.boolean "enabled", default: true, null: false
    t.string "name", null: false
    t.jsonb "trigger_config", default: {}
    t.string "trigger_type", null: false
    t.datetime "updated_at", null: false
    t.string "webhook_token"
    t.uuid "workspace_id", null: false
    t.index ["created_by_id"], name: "index_workflows_on_created_by_id"
    t.index ["webhook_token"], name: "index_workflows_on_webhook_token", unique: true
    t.index ["workspace_id"], name: "index_workflows_on_workspace_id"
  end

  create_table "workspaces", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.boolean "ai_processing_enabled", default: true, null: false
    t.datetime "created_at", null: false
    t.integer "email_retention_months"
    t.jsonb "entitlement_overrides", default: {}, null: false
    t.string "name", null: false
    t.string "plan", default: "free", null: false
    t.string "required_data_region"
    t.boolean "scout_thread_posts", default: false, null: false
    t.jsonb "settings", default: {}, null: false
    t.string "slug", null: false
    t.datetime "updated_at", null: false
    t.string "workspace_type", default: "company", null: false
    t.index ["plan"], name: "index_workspaces_on_plan"
    t.index ["slug"], name: "index_workspaces_on_slug", unique: true
    t.check_constraint "workspace_type::text = ANY (ARRAY['company'::character varying::text, 'individual'::character varying::text])", name: "chk_organizations_workspace_type"
  end

  create_table "zoho_drive_accounts", id: :uuid, default: -> { "gen_random_uuid()" }, force: :cascade do |t|
    t.boolean "active", default: true, null: false
    t.datetime "created_at", null: false
    t.string "email_address", null: false
    t.datetime "last_synced_at"
    t.datetime "updated_at", null: false
    t.uuid "workspace_id"
    t.string "zoho_account_id"
    t.text "zoho_refresh_token", null: false
    t.index ["email_address"], name: "index_zoho_drive_accounts_on_email_address", unique: true
    t.index ["workspace_id"], name: "index_zoho_drive_accounts_on_workspace_id"
  end

  add_foreign_key "account_exports", "users"
  add_foreign_key "active_storage_attachments", "active_storage_blobs", column: "blob_id"
  add_foreign_key "active_storage_variant_records", "active_storage_blobs", column: "blob_id"
  add_foreign_key "agent_messages", "agent_threads"
  add_foreign_key "agent_messages", "users"
  add_foreign_key "agent_threads", "users"
  add_foreign_key "agent_threads", "workspaces"
  add_foreign_key "ai_adapters", "workspaces"
  add_foreign_key "ai_configurations", "ai_adapters"
  add_foreign_key "ai_configurations", "workspaces"
  add_foreign_key "ai_prompts", "workspaces"
  add_foreign_key "audit_events", "users", on_delete: :nullify
  add_foreign_key "authored_documents", "users", column: "author_id"
  add_foreign_key "authored_documents", "workspaces"
  add_foreign_key "beta_codes", "users", column: "created_by_id"
  add_foreign_key "beta_codes", "users", column: "redeemed_by_id"
  add_foreign_key "bug_reports", "users"
  add_foreign_key "bug_reports", "workspaces"
  add_foreign_key "calendar_account_users", "calendar_accounts"
  add_foreign_key "calendar_account_users", "users"
  add_foreign_key "calendar_accounts", "workspaces"
  add_foreign_key "calendar_events", "calendars"
  add_foreign_key "calendar_events", "email_messages", column: "source_email_message_id", on_delete: :nullify
  add_foreign_key "calendar_events", "event_types"
  add_foreign_key "calendar_sync_logs", "calendar_accounts"
  add_foreign_key "calendar_webhook_channels", "calendars"
  add_foreign_key "calendars", "calendar_accounts"
  add_foreign_key "connections", "workspaces"
  add_foreign_key "contact_email_aliases", "contacts"
  add_foreign_key "contact_tags", "contacts"
  add_foreign_key "contact_tags", "tags"
  add_foreign_key "contacts", "contacts", column: "duplicate_of_id"
  add_foreign_key "contacts", "email_accounts"
  add_foreign_key "contacts", "people"
  add_foreign_key "contacts", "people", column: "suggested_person_id"
  add_foreign_key "contacts", "workspaces"
  add_foreign_key "devices", "users"
  add_foreign_key "digest_issues", "scheduled_digests"
  add_foreign_key "document_drive_uploads", "documents"
  add_foreign_key "document_drive_uploads", "zoho_drive_accounts"
  add_foreign_key "document_email_messages", "documents"
  add_foreign_key "document_email_messages", "email_messages"
  add_foreign_key "document_templates", "workspaces"
  add_foreign_key "document_types", "workspaces"
  add_foreign_key "documents", "document_types"
  add_foreign_key "documents", "email_accounts"
  add_foreign_key "documents", "users", column: "reviewed_by_id"
  add_foreign_key "documents", "workspaces"
  add_foreign_key "draft_emails", "email_accounts"
  add_foreign_key "draft_emails", "email_messages", column: "in_reply_to_id"
  add_foreign_key "draft_emails", "signatures"
  add_foreign_key "draft_emails", "users"
  add_foreign_key "draft_emails", "workspaces"
  add_foreign_key "drive_folder_mappings", "document_types"
  add_foreign_key "drive_folder_mappings", "zoho_drive_accounts"
  add_foreign_key "email_account_signatures", "email_accounts"
  add_foreign_key "email_account_signatures", "signatures"
  add_foreign_key "email_account_users", "email_accounts"
  add_foreign_key "email_account_users", "users"
  add_foreign_key "email_accounts", "workspaces"
  add_foreign_key "email_folders", "email_accounts"
  add_foreign_key "email_message_tags", "email_messages"
  add_foreign_key "email_message_tags", "tags"
  add_foreign_key "email_messages", "agent_messages", column: "ai_analysis_message_id"
  add_foreign_key "email_messages", "contacts"
  add_foreign_key "email_messages", "email_accounts"
  add_foreign_key "email_messages", "email_scan_logs"
  add_foreign_key "email_messages", "email_threads"
  add_foreign_key "email_scan_logs", "email_accounts"
  add_foreign_key "email_template_documents", "document_templates"
  add_foreign_key "email_template_documents", "email_templates"
  add_foreign_key "email_templates", "workspaces"
  add_foreign_key "email_threads", "email_accounts"
  add_foreign_key "event_types", "workspaces"
  add_foreign_key "events", "events", column: "caused_by_event_id"
  add_foreign_key "events", "workspaces"
  add_foreign_key "exports", "workspaces"
  add_foreign_key "feed_items", "users"
  add_foreign_key "feed_items", "workspaces"
  add_foreign_key "file_share_links", "users", column: "created_by_id"
  add_foreign_key "file_share_links", "workspaces"
  add_foreign_key "folder_memberships", "mail_folders"
  add_foreign_key "google_drive_accounts", "workspaces"
  add_foreign_key "google_drive_configs", "document_types"
  add_foreign_key "identities", "users"
  add_foreign_key "invitations", "users", column: "accepted_by_id"
  add_foreign_key "invitations", "users", column: "invited_by_id"
  add_foreign_key "invitations", "workspaces"
  add_foreign_key "learning_decisions", "contacts"
  add_foreign_key "learning_decisions", "users"
  add_foreign_key "learning_decisions", "workspaces"
  add_foreign_key "mail_folder_users", "mail_folders"
  add_foreign_key "mail_folder_users", "users"
  add_foreign_key "mail_folders", "mail_folders", column: "parent_id"
  add_foreign_key "mail_folders", "workspaces"
  add_foreign_key "mfa_email_challenges", "users"
  add_foreign_key "notification_preferences", "document_types"
  add_foreign_key "notification_preferences", "tags"
  add_foreign_key "notification_preferences", "users"
  add_foreign_key "notifications", "users"
  add_foreign_key "notion_database_mappings", "document_types"
  add_foreign_key "notion_integrations", "users", column: "authorized_by_user_id"
  add_foreign_key "notion_integrations", "workspaces"
  add_foreign_key "notion_pages", "documents"
  add_foreign_key "notion_pages", "notion_database_mappings"
  add_foreign_key "oauth_access_grants", "oauth_applications", column: "application_id"
  add_foreign_key "oauth_access_tokens", "oauth_applications", column: "application_id"
  add_foreign_key "oauth_applications", "users", column: "created_by_id"
  add_foreign_key "oauth_applications", "workspaces"
  add_foreign_key "organization_memberships", "organizations"
  add_foreign_key "organization_memberships", "people"
  add_foreign_key "organizations", "workspaces"
  add_foreign_key "people", "workspaces"
  add_foreign_key "pipeline_memberships", "pipeline_stages", column: "current_stage_id", on_delete: :nullify
  add_foreign_key "pipeline_memberships", "pipelines"
  add_foreign_key "pipeline_stages", "pipelines"
  add_foreign_key "pipelines", "workspaces"
  add_foreign_key "recovery_codes", "users"
  add_foreign_key "reminders", "calendar_events"
  add_foreign_key "reminders", "users", column: "confirmed_by_id"
  add_foreign_key "reminders", "workspaces"
  add_foreign_key "scheduled_digests", "users"
  add_foreign_key "scheduled_digests", "workspaces"
  add_foreign_key "scheduled_emails", "email_accounts"
  add_foreign_key "scheduled_emails", "email_templates"
  add_foreign_key "scheduled_emails", "users", column: "created_by_id"
  add_foreign_key "scheduled_emails", "workspaces"
  add_foreign_key "search_chunks", "workspaces"
  add_foreign_key "search_records", "workspaces"
  add_foreign_key "search_tag_embeddings", "tags"
  add_foreign_key "search_tag_embeddings", "workspaces"
  add_foreign_key "sessions", "users"
  add_foreign_key "signatures", "users"
  add_foreign_key "signup_requests", "users", column: "accepted_by_id"
  add_foreign_key "signup_requests", "users", column: "reviewed_by_id"
  add_foreign_key "skim_decisions", "contacts"
  add_foreign_key "skim_decisions", "users"
  add_foreign_key "skim_decisions", "workspaces"
  add_foreign_key "solid_queue_blocked_executions", "solid_queue_jobs", column: "job_id", on_delete: :cascade
  add_foreign_key "solid_queue_claimed_executions", "solid_queue_jobs", column: "job_id", on_delete: :cascade
  add_foreign_key "solid_queue_failed_executions", "solid_queue_jobs", column: "job_id", on_delete: :cascade
  add_foreign_key "solid_queue_ready_executions", "solid_queue_jobs", column: "job_id", on_delete: :cascade
  add_foreign_key "solid_queue_recurring_executions", "solid_queue_jobs", column: "job_id", on_delete: :cascade
  add_foreign_key "solid_queue_scheduled_executions", "solid_queue_jobs", column: "job_id", on_delete: :cascade
  add_foreign_key "tags", "workspaces"
  add_foreign_key "task_assignments", "tasks"
  add_foreign_key "task_assignments", "users"
  add_foreign_key "task_assignments", "users", column: "assigned_by_id", on_delete: :nullify
  add_foreign_key "task_documents", "documents"
  add_foreign_key "task_documents", "tasks"
  add_foreign_key "task_documents", "users", column: "created_by_id", on_delete: :nullify
  add_foreign_key "task_email_links", "email_messages"
  add_foreign_key "task_email_links", "tasks"
  add_foreign_key "task_email_links", "users", column: "created_by_id", on_delete: :nullify
  add_foreign_key "task_tags", "tags"
  add_foreign_key "task_tags", "tasks"
  add_foreign_key "tasks", "tasks", column: "recurrence_parent_id", on_delete: :nullify
  add_foreign_key "tasks", "users", column: "created_by_id", on_delete: :nullify
  add_foreign_key "tasks", "workspaces"
  add_foreign_key "thread_follows", "agent_threads"
  add_foreign_key "thread_follows", "users"
  add_foreign_key "users", "workspaces"
  add_foreign_key "webauthn_credentials", "users"
  add_foreign_key "workflow_execution_steps", "workflow_executions"
  add_foreign_key "workflow_execution_steps", "workflow_steps"
  add_foreign_key "workflow_executions", "workflows"
  add_foreign_key "workflow_executions", "workspaces"
  add_foreign_key "workflow_steps", "workflows"
  add_foreign_key "workflows", "users", column: "created_by_id"
  add_foreign_key "workflows", "workspaces"
  add_foreign_key "zoho_drive_accounts", "workspaces"
end

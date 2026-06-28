# frozen_string_literal: true

# Convert every domain table's primary key from bigint to uuid, and rewrite
# every column that references one (real foreign keys, convention/manual
# foreign keys without a DB constraint, and polymorphic `*_id` columns).
#
# Runs in a single transaction (PostgreSQL DDL is transactional): any failure
# rolls the database all the way back to bigint, with no partial/orphaned state.
#
# Reference columns come from three sources:
#   1. Real FK constraints — discovered by introspection (`foreign_keys`), so no
#      Rails-generated constraint hash names are ever hardcoded.
#   2. EXTRA_FKS — columns that reference a domain table by `belongs_to` or by
#      hand but carry no DB constraint, so introspection can't see them. Kept as
#      an explicit, reviewed list (a spec guards it against drift).
#   3. POLYMORPHIC — `*_id` columns (no constraint, possibly many target types),
#      backfilled by joining on the paired `*_type`. Includes framework tables
#      (Active Storage attachments, Action Text) that attach to domain models.
#
# `documents.email_message_id` is intentionally NOT migrated: it references
# email_messages by `provider_message_id` (a string), not by the primary key.
#
# Irreversible: original bigint ids cannot be reconstructed once dropped.
class MigratePrimaryKeysToUuid < ActiveRecord::Migration[8.1]
  # Domain tables whose bigint `id` becomes a uuid. Excludes framework tables
  # (Solid Queue/Cable, Active Storage, Action Text, Doorkeeper token/grant
  # tables), tables already on uuid (organizations, organization_memberships),
  # and search_chunks/search_records — large, regenerable pgvector tables that
  # nothing references by `id`, so their PK stays bigint to avoid a multi-GB
  # table rewrite. Their `searchable_id`/`workspace_id` columns are still moved
  # to uuid (via POLYMORPHIC + FK introspection) so lookups keep resolving.
  DOMAIN_TABLES = %w[
    account_exports agent_messages agent_threads ai_adapters ai_configurations
    audit_events authored_documents beta_codes bug_reports calendar_account_users
    calendar_accounts calendar_events calendar_sync_logs calendar_webhook_channels
    calendars connections contact_email_aliases contact_tags contacts devices
    document_drive_uploads document_email_messages document_templates
    document_types documents drive_folder_mappings email_account_signatures
    email_account_users email_accounts email_folders email_message_tags
    email_messages email_scan_logs email_threads events exports feed_items
    folder_memberships google_drive_accounts google_drive_configs identities
    invitations mail_folders mfa_email_challenges notification_preferences
    notifications notion_database_mappings notion_integrations notion_pages
    oauth_applications people pipeline_memberships pipeline_stages pipelines
    recovery_codes reminders scheduled_emails search_tag_embeddings sessions
    signatures signup_requests skim_decisions tags templates thread_follows
    users webauthn_credentials
    workflow_execution_steps workflow_executions workflow_steps workflows
    workspaces zoho_drive_accounts
  ].freeze

  # `[from_table, column, to_table]` references to a domain table that have NO DB
  # foreign-key constraint (so `foreign_keys` can't find them): convention FKs
  # (a `belongs_to` Rails infers) and hand-maintained id columns. Their type is
  # migrated and values backfilled, but no new constraint is added (preserving
  # the original schema's choice not to constrain them).
  EXTRA_FKS = [
    %w[tags email_account_id email_accounts],
    %w[email_threads follow_up_outbound_message_id email_messages],
    %w[skim_decisions email_message_id email_messages],
    %w[oauth_access_grants resource_owner_id users],
    %w[oauth_access_tokens resource_owner_id users]
  ].freeze

  # Polymorphic `*_id` columns (no constraint) that store a DOMAIN_TABLES PK.
  # Each is `[table, id_column]`; the paired `*_type` column drives the backfill.
  POLYMORPHIC = [
    %w[action_text_rich_texts record_id],
    %w[active_storage_attachments record_id],
    %w[agent_threads contextable_id],
    %w[audit_events target_id],
    %w[events actor_id],
    %w[events subject_id],
    %w[feed_items subject_id],
    %w[folder_memberships folderable_id],
    %w[notifications notifiable_id],
    %w[pipeline_memberships item_id],
    %w[reminders source_id],
    %w[search_chunks searchable_id],
    %w[search_records searchable_id]
  ].freeze

  # One reference column to rewrite: a real FK (constrained) or an EXTRA_FK.
  Ref = Struct.new(:from_table, :column, :to_table, :on_delete, :constraint_name, keyword_init: true) do
    def constrained? = !constraint_name.nil?
  end

  def up
    # Convert only tables that exist. A schema can lag its migrations (a table
    # added by a migration newer than the loaded schema — e.g. document_templates
    # on a fresh v0.3.0 install), so guard every table op rather than assume all
    # of DOMAIN_TABLES is present. On a migrated database every table is there.
    tables = DOMAIN_TABLES.select { |t| connection.table_exists?(t) }
    poly   = POLYMORPHIC.select { |t, _| connection.table_exists?(t) }
    domain = tables.to_set

    refs = connection.tables.flat_map { |t| connection.foreign_keys(t) }
                     .select { |fk| domain.include?(fk.to_table) }
                     .map { |fk| Ref.new(from_table: fk.from_table, column: fk.column, to_table: fk.to_table, on_delete: fk.on_delete, constraint_name: fk.name) }
    EXTRA_FKS.each do |from, col, to|
      next unless connection.column_exists?(from, col)

      refs << Ref.new(from_table: from, column: col, to_table: to, on_delete: nil, constraint_name: nil)
    end

    # Snapshot indexes on every table we touch, so we can restore any dropped
    # when their underlying FK/polymorphic column is dropped.
    touched = (tables + refs.map(&:from_table) + poly.map(&:first)).uniq
    indexes_before = touched.index_with { |t| connection.indexes(t) }

    # NOT NULL flags to restore on rewritten reference columns.
    ref_not_null  = refs.index_with { |r| column_not_null?(r.from_table, r.column) }
    poly_not_null = poly.index_with { |(t, c)| column_not_null?(t, c) }

    say_with_time "Add uuid columns to #{tables.size} domain tables" do
      tables.each do |t|
        add_column t, :id_new, :uuid, default: -> { "gen_random_uuid()" }, null: false
      end
    end

    say_with_time "Backfill #{refs.size} reference columns" do
      refs.each do |r|
        add_column r.from_table, "#{r.column}_new", :uuid
        execute(<<~SQL.squish)
          UPDATE #{qt r.from_table} AS child
          SET #{qc "#{r.column}_new"} = parent.id_new
          FROM #{qt r.to_table} AS parent
          WHERE child.#{qc r.column} = parent.id
        SQL
      end
    end

    say_with_time "Backfill #{poly.size} polymorphic columns" do
      poly.each do |table, col|
        add_column table, "#{col}_new", :uuid
        type_col = col.sub(/_id\z/, "_type")
        tables.each do |dt|
          execute(<<~SQL.squish)
            UPDATE #{qt table} AS child
            SET #{qc "#{col}_new"} = parent.id_new
            FROM #{qt dt} AS parent
            WHERE child.#{qc type_col} = #{connection.quote(dt.classify)}
              AND child.#{qc col} = parent.id
          SQL
        end
      end
    end

    say_with_time "Drop foreign-key constraints" do
      refs.select(&:constrained?).each { |r| remove_foreign_key r.from_table, name: r.constraint_name }
    end

    say_with_time "Promote uuid columns to primary keys" do
      tables.each do |t|
        execute "ALTER TABLE #{qt t} DROP CONSTRAINT #{qc "#{t}_pkey"}"
        remove_column t, :id           # also drops the owned bigint sequence
        rename_column t, :id_new, :id
        execute "ALTER TABLE #{qt t} ADD PRIMARY KEY (id)"
      end
    end

    say_with_time "Swap reference columns into place" do
      poly_refs = poly.map { |t, c| Ref.new(from_table: t, column: c) }
      (refs + poly_refs).each do |r|
        remove_column r.from_table, r.column
        rename_column r.from_table, "#{r.column}_new", r.column
      end
      refs.each { |r| change_column_null r.from_table, r.column, false if ref_not_null[r] }
      poly.each { |t, c| change_column_null t, c, false if poly_not_null[[ t, c ]] }
    end

    say_with_time "Recreate foreign-key constraints" do
      refs.select(&:constrained?).each do |r|
        add_foreign_key r.from_table, r.to_table, column: r.column, on_delete: r.on_delete
      end
    end

    say_with_time "Restore indexes dropped with their columns" do
      touched.each do |t|
        existing = connection.indexes(t).map(&:name).to_set
        indexes_before[t].each { |idx| recreate_index(t, idx) unless existing.include?(idx.name) }
      end
    end
  end

  def down
    raise ActiveRecord::IrreversibleMigration,
          "Original bigint primary keys cannot be reconstructed from uuids."
  end

  private

  def qt(name) = connection.quote_table_name(name)
  def qc(name) = connection.quote_column_name(name)

  def column_not_null?(table, column)
    col = connection.columns(table).find { |c| c.name == column }
    col && !col.null
  end

  def recreate_index(table, idx)
    options = { name: idx.name, unique: idx.unique }
    options[:where] = idx.where if idx.where
    options[:using] = idx.using if idx.using
    options[:order] = idx.orders if idx.orders.present?
    add_index table, idx.columns, **options
  end
end

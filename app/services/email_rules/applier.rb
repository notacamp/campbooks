# frozen_string_literal: true

module EmailRules
  # Applies a workspace's enabled rules to an email at ingest time, or applies
  # a single rule's actions to an email in a retroactive run batch.
  #
  # Job-safe: no dependence on Current.user.
  #
  # Ingest usage (EmailProcessJob hook):
  #   EmailRules::Applier.new(email).call
  #
  # Retroactive run usage (EmailRuleRunJob, per email in the batch — the shared
  # archive_folder_cache avoids re-resolving the provider archive folder per
  # email; Zoho resolves it via a folder-list API call):
  #   EmailRules::Applier.new(email, archive_folder_cache: cache).apply(rule, run: run_record)
  class Applier
    def initialize(email, archive_folder_cache: {})
      @email = email
      @archive_folder_cache = archive_folder_cache
    end

    # Loads all enabled rules for the workspace and applies any that match.
    # Called once per newly ingested email.  rule.matched_count counts these
    # ingest-time matches only; retroactive sweeps report via their run record.
    def call
      workspace = @email.email_account.workspace
      rules = workspace.email_rules.enabled.includes(:tags, :mail_folder)

      rules.each do |rule|
        matcher = EmailRules::Matcher.new(rule)
        next unless matcher.matches?(@email)

        apply(rule)
        EmailRule.increment_counter(:matched_count, rule.id)
      rescue => e
        Rails.logger.error(
          "[EmailRules::Applier] rule #{rule.id} failed for email #{@email.id}: #{e.message}"
        )
      end
    end

    # Applies a single rule's actions to @email.  Idempotent: each action is a
    # no-op when the state is already applied.  When `run:` is given, undo
    # bookkeeping is appended to the run's id arrays (only for state this call
    # actually changed, and only while the run is undoable).
    def apply(rule, run: nil)
      apply_tags(rule, run: run)
      apply_archive(rule, run: run)
      apply_mark_read(rule, run: run)
      apply_folder(rule, run: run)
    end

    private

    # Tags: find-or-create with provenance.  The unique constraint on
    # (email_message_id, tag_id) makes this idempotent.  On create, the row
    # records which rule applied the tag (applied_by_rule_id) so UndoRun can
    # target it precisely.  If the tag was already applied by another path the
    # existing row is found (without overwriting its applied_by_rule_id).
    def apply_tags(rule, run: nil)
      created_any = false

      rule.tags.each do |tag|
        row = @email.email_message_tags
          .create_with(applied_by_rule_id: rule.id)
          .find_or_create_by!(tag: tag)
        created_any ||= row.previously_new_record?
      end

      record_undo(run, :tagged_email_ids) if created_any
    end

    # Archive: reuse the exact same path as the blocked-sender auto-archive in
    # EmailProcessJob#apply_sender_rules -> Tools::Archive.call(email), which
    # archives the whole thread at the provider and mirrors the folder locally.
    #
    # Already-archived detection compares provider_folder_id against the
    # provider's archive folder id (resolved once per account via the shared
    # cache).  Bookkeeping only records the email when this call actually
    # archived it (Tools::Archive returns nil on failure), so Undo never pulls
    # mail into the inbox that the run didn't move.
    def apply_archive(rule, run: nil)
      return unless rule.archive?

      return if @email.provider_folder_id.present? &&
                @email.provider_folder_id == archive_folder_id_for(@email.email_account)

      result = Tools::Archive.call(@email)
      record_undo(run, :archived_email_ids) if result
    end

    # Mark read: mirrors Tools::BulkMarkRead for a single email, job-safe
    # (no Current.user).  Updates the DB column immediately and pushes the read
    # state to the provider via MarkReadJob (same as BulkMarkRead does).
    def apply_mark_read(rule, run: nil)
      return unless rule.mark_read?
      return if @email.read?

      @email.update_columns(read: true, updated_at: Time.current)
      if @email.provider_message_id.present?
        MarkReadJob.perform_later(@email.email_account_id, [ @email.provider_message_id ])
      end

      record_undo(run, :marked_read_email_ids)
    end

    # Folder membership: creates a FolderMembership row (the local filesystem
    # layer, same as Tools::BulkMoveToFolder#record_email_memberships).
    # No-op when the membership already exists (find_or_create_by idempotence).
    def apply_folder(rule, run: nil)
      return unless rule.mail_folder_id.present?

      folder = rule.mail_folder
      return unless folder

      membership = folder.folder_memberships
        .find_or_create_by!(folderable_type: "EmailMessage", folderable_id: @email.id)

      record_undo(run, :moved_email_ids) if membership.previously_new_record?
    end

    def record_undo(run, array_attr)
      return unless run&.undoable?

      run[array_attr] = (run[array_attr] || []) + [ @email.id ]
    end

    # The provider's archive folder id, resolved once per account.  Zoho needs
    # a folder-list API call; Google is the constant "ARCHIVE".
    def archive_folder_id_for(account)
      @archive_folder_cache[account.id] ||= account.mail_client.archive_folder_id
    rescue => e
      Rails.logger.error("[EmailRules::Applier] archive folder lookup failed for account #{account.id}: #{e.message}")
      nil
    end
  end
end

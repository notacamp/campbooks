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
  # Retroactive run usage (EmailRuleRunJob, per email in the batch):
  #   EmailRules::Applier.new(email).apply(rule, run: run_record)
  class Applier
    def initialize(email)
      @email = email
    end

    # Loads all enabled rules for the workspace and applies any that match.
    # Called once per newly ingested email.
    def call
      workspace = @email.email_account.workspace
      rules = workspace.email_rules.enabled.includes(:tags, :mail_folder)

      rules.each do |rule|
        matcher = EmailRules::Matcher.new(rule)
        next unless matcher.matches?(@email)

        apply(rule)
      rescue => e
        Rails.logger.error(
          "[EmailRules::Applier] rule #{rule.id} failed for email #{@email.id}: #{e.message}"
        )
      end
    end

    # Applies a single rule's actions to @email.  Idempotent: each action is a
    # no-op when the state is already applied.  When `run:` is given, undo
    # bookkeeping is appended to the run's id arrays (only if undoable is true).
    def apply(rule, run: nil)
      apply_tags(rule, run: run)
      apply_archive(rule, run: run)
      apply_mark_read(rule, run: run)
      apply_folder(rule, run: run)
      EmailRule.increment_counter(:matched_count, rule.id)
    end

    private

    # Tags: find-or-create with provenance.  The unique constraint on
    # (email_message_id, tag_id) makes this idempotent.  On create, the row
    # records which rule applied the tag (applied_by_rule_id) so UndoRun can
    # target it precisely.  If the tag was already applied by another path the
    # existing row is found (without overwriting its applied_by_rule_id).
    def apply_tags(rule, run: nil)
      rule.tags.each do |tag|
        @email.email_message_tags
          .create_with(applied_by_rule_id: rule.id)
          .find_or_create_by!(tag: tag)
      end
    end

    # Archive: reuse the exact same path as the blocked-sender auto-archive in
    # EmailProcessJob#apply_sender_rules -> Tools::Archive.call(email).
    # Only moves the thread when it is not already in the archive folder (to
    # avoid a provider round-trip and to keep the undo arrays accurate).
    # The "archive folder" is resolved from the local email_folders mirror
    # (no extra API call); falls back to always-archive if the mirror row is
    # absent.
    def apply_archive(rule, run: nil)
      return unless rule.archive?

      account = @email.email_account
      archive_row = account.email_folders.find_by("LOWER(name) = ?", "archive")
      already_archived = archive_row.present? &&
        @email.provider_folder_id == archive_row.provider_folder_id

      return if already_archived

      Tools::Archive.call(@email)

      if run&.undoable?
        run.archived_email_ids = (run.archived_email_ids || []) + [ @email.id ]
      end
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

      if run&.undoable?
        run.marked_read_email_ids = (run.marked_read_email_ids || []) + [ @email.id ]
      end
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

      if membership.previously_new_record? && run&.undoable?
        run.moved_email_ids = (run.moved_email_ids || []) + [ @email.id ]
      end
    end
  end
end

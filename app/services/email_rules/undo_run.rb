# frozen_string_literal: true

module EmailRules
  # Reverses a completed EmailRuleRun.  For each action the run applied, the
  # inverse is performed:
  #   - Tags the RUN applied are removed — scoped by provenance
  #     (applied_by_rule_id) AND the run's tagged_email_ids, so tags the user
  #     added themselves and tags this rule applied at ingest time stay.
  #   - Threads the run archived are moved back to the inbox (grouped per
  #     account, one provider call per batch — mirrors Tools::BulkUnarchive).
  #   - Emails the run marked read are marked unread (DB + provider job).
  #   - Folder memberships the run created are removed.
  #
  # Refuses to run when the run is not completed or undoable is false.
  class UndoRun
    def self.call(run)
      new(run).call
    end

    def initialize(run)
      @run = run
    end

    def call
      unless @run.completed? && @run.undoable?
        raise ArgumentError,
              "run #{@run.id} cannot be undone " \
              "(status=#{@run.status}, undoable=#{@run.undoable})"
      end

      remove_rule_tags
      unarchive_emails
      mark_emails_unread
      remove_folder_memberships

      @run.update!(status: :undone)
    end

    private

    def rule
      @run.email_rule
    end

    # Remove tag rows whose provenance points to this rule, limited to the
    # emails THIS run tagged.  Ingest-time applications of the same rule (which
    # record provenance but are not part of the run) are untouched.
    def remove_rule_tags
      email_ids = @run.tagged_email_ids
      return if email_ids.blank?

      EmailMessageTag.where(
        applied_by_rule_id: rule.id,
        email_message_id: email_ids
      ).destroy_all
    end

    # Move archived mail back to the inbox.  Tools::Archive moved the whole
    # thread, so the undo restores the whole thread too.  Provider calls are
    # grouped per account (one move_to_folder per account batch), mirroring
    # Tools::BulkUnarchive rather than one call per email.
    def unarchive_emails
      email_ids = @run.archived_email_ids
      return if email_ids.blank?

      emails = EmailMessage.where(id: email_ids).includes(:email_account, :email_thread)

      emails.group_by(&:email_account).each do |account, messages|
        client = account.mail_client
        next unless client.respond_to?(:inbox_folder_id) && client.respond_to?(:move_to_folder)

        inbox_id = client.inbox_folder_id
        next unless inbox_id

        thread_ids = messages.map(&:email_thread_id).compact.uniq
        scope = account.email_messages.where(email_thread_id: thread_ids)
          .or(account.email_messages.where(id: messages.map(&:id)))

        provider_ids = scope.pluck(:provider_message_id).compact
        next if provider_ids.empty?

        client.move_to_folder(provider_ids, inbox_id)
        scope.update_all(provider_folder_id: inbox_id, updated_at: Time.current)
      rescue => e
        Rails.logger.error("[EmailRules::UndoRun] unarchive failed for account #{account.id}: #{e.message}")
      end
    end

    # Mark emails unread in the DB, then push the state to the provider with
    # one MarkUnreadJob per account (mirrors BulkMarkRead's job-dispatch
    # pattern, batched instead of per-email).
    def mark_emails_unread
      email_ids = @run.marked_read_email_ids
      return if email_ids.blank?

      emails = EmailMessage.where(id: email_ids)
      emails.update_all(read: false, updated_at: Time.current)

      emails.group_by(&:email_account_id).each do |account_id, messages|
        provider_ids = messages.map(&:provider_message_id).compact
        next if provider_ids.empty?

        MarkUnreadJob.perform_later(account_id, provider_ids)
      end
    end

    # Remove folder memberships the run created for the rule's target folder.
    def remove_folder_memberships
      email_ids = @run.moved_email_ids
      return if email_ids.blank?
      return unless rule.mail_folder_id.present?

      FolderMembership.where(
        mail_folder_id: rule.mail_folder_id,
        folderable_type: "EmailMessage",
        folderable_id: email_ids
      ).destroy_all
    end
  end
end

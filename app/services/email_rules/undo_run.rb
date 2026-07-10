# frozen_string_literal: true

module EmailRules
  # Reverses a completed EmailRuleRun.  For each action the run applied, the
  # inverse is performed:
  #   - Tags applied by the rule are removed (by provenance: applied_by_rule_id).
  #   - Archived emails are moved back to the inbox folder (per account).
  #   - Emails marked read by the run are marked unread (DB + provider).
  #   - Folder memberships added by the run are removed.
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

    # Remove tag rows whose provenance points to this rule.  Scoped by
    # applied_by_rule_id so only rule-applied tags are removed; tags the user
    # added independently are untouched.
    def remove_rule_tags
      EmailMessageTag.where(applied_by_rule_id: rule.id).destroy_all
    end

    # Move archived emails back to the inbox folder, per account (mirrors
    # Tools::BulkUnarchive, but without Current.user dependency).
    def unarchive_emails
      email_ids = @run.archived_email_ids
      return if email_ids.blank?

      EmailMessage.where(id: email_ids).includes(:email_account).find_each do |email|
        account = email.email_account
        client  = account.mail_client
        next unless client.respond_to?(:inbox_folder_id) && client.respond_to?(:move_to_folder)

        inbox_id = client.inbox_folder_id
        next unless inbox_id

        provider_ids = [ email.provider_message_id ].compact
        next if provider_ids.empty?

        client.move_to_folder(provider_ids, inbox_id)
        email.update_columns(provider_folder_id: inbox_id, updated_at: Time.current)
      rescue => e
        Rails.logger.error("[EmailRules::UndoRun] unarchive failed for email #{email.id}: #{e.message}")
      end
    end

    # Mark emails unread in DB and push the state to the provider via
    # MarkUnreadJob (mirrors BulkMarkRead's job-dispatch pattern).
    def mark_emails_unread
      email_ids = @run.marked_read_email_ids
      return if email_ids.blank?

      EmailMessage.where(id: email_ids).find_each do |email|
        email.update_columns(read: false, updated_at: Time.current)
        if email.provider_message_id.present?
          MarkUnreadJob.perform_later(email.email_account_id, [ email.provider_message_id ])
        end
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

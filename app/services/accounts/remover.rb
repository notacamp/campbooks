module Accounts
  # Full teardown of ONE email account — the user-initiated "Remove account"
  # action in Settings → Accounts. Destroys everything mail-specific (messages
  # and their attachments, scan logs, threads, folders, the account's provider
  # tags, scheduled sends, legacy local drafts) and DETACHES the derived
  # documents and contacts: their email_account_id is nulled so filed paperwork
  # and saved contacts outlive the mailbox (mirrors Contact#promote_to_global!).
  # Best-effort revokes the OAuth grant at the provider, but only when no
  # still-connected sibling (e.g. the calendar from the same grant) shares the
  # token — that check is delegated to Accounts::TokenRevoker.
  #
  # This is the single-account analogue of Accounts::Deleter#delete_sole_member!,
  # and like it runs off-request (EmailAccountRemovalJob) so a large mailbox
  # can't time out the click. A plain EmailAccount#destroy is deliberately
  # blocked today (email_messages/email_scan_logs use :restrict_with_error, and
  # contacts/documents/draft_emails/scheduled_emails hold DB foreign keys with no
  # cascade), so the order below clears every dependent before the account row.
  class Remover
    def initialize(account)
      @account = account
    end

    def remove!
      # Drop it from the active set before the unshared-token check, so
      # TokenRevoker doesn't count the account as its own still-connected sibling
      # and skip the revoke. The controller already did this for the instant UI
      # effect; repeating it keeps the service correct when called directly.
      # update_columns skips the after_update "disconnected" notifier.
      @account.update_columns(active: false) if @account.active?

      # Network I/O — must run outside the delete transaction. Best-effort:
      # TokenRevoker swallows its own errors and only revokes when unshared.
      Accounts::TokenRevoker.revoke_if_unshared(@account)

      ApplicationRecord.transaction do
        detach_kept_records
        purge_mail_specific_children
        # destroy_all (not delete_all) so each message runs its dependent
        # cascades — email_message_tags, document_email_messages, folder
        # memberships, task links — and purges its ActiveStorage attachments.
        @account.email_messages.destroy_all
        # restrict_with_error would veto the account destroy, so clear the scan
        # logs directly. Use the class relation, not the association — the
        # latter's delete_all *nullifies* the FK (which is NOT NULL) instead of
        # deleting the rows.
        EmailScanLog.where(email_account_id: @account.id).delete_all
        # Cascades the remaining has_many dependents declared on EmailAccount:
        # threads (→ agent_thread → agent_messages), folders, external tags,
        # account-user shares, and signature links.
        @account.destroy!
      end
    end

    private

    # Keep the user's curated data — null its link to the mailbox so it survives.
    def detach_kept_records
      Document.where(email_account_id: @account.id).update_all(email_account_id: nil)
      Contact.where(email_account_id: @account.id).update_all(email_account_id: nil)
    end

    # Mail-only records that hold a foreign key to the account (or its messages)
    # but have no dependent cascade, so they'd otherwise block the deletes.
    def purge_mail_specific_children
      ScheduledEmail.where(email_account_id: @account.id).delete_all

      # draft_emails is a legacy table with no model (drafts are saved
      # provider-side now) but keeps live FKs to both email_accounts and
      # email_messages(in_reply_to_id). Clear any stragglers so neither the
      # message nor the account destroy trips the constraint.
      draft_emails = Class.new(ApplicationRecord) { self.table_name = "draft_emails" }
      draft_emails.where(email_account_id: @account.id).delete_all
      draft_emails.where(in_reply_to_id: @account.email_messages.select(:id)).delete_all
    end
  end
end

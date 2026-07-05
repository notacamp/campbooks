module Accounts
  class Deleter
    def initialize(user)
      @user = user
      @workspace = user.workspace
    end

    def delete!
      if @workspace.nil?
        # Workspace-less user (edge case): nullify FK landmines and remove the user.
        nullify_fk_landmines(@user)
        @user.destroy!
        return
      end

      if @workspace.users.count == 1
        delete_sole_member!
      else
        delete_member!
      end
    end

    private

    def delete_sole_member!
      revoke_tokens_best_effort
      purge_attachments

      ApplicationRecord.transaction do
        @workspace.documents.destroy_all

        email_account_ids = @workspace.email_accounts.ids
        EmailMessage.where(email_account_id: email_account_ids).destroy_all
        EmailScanLog.where(email_account_id: email_account_ids).delete_all
        @workspace.email_accounts.destroy_all

        calendar_account_ids = @workspace.calendar_accounts.ids
        CalendarSyncLog.where(calendar_account_id: calendar_account_ids).delete_all
        @workspace.calendar_accounts.destroy_all

        ExternalServiceCall.where(workspace_id: @workspace.id).in_batches.delete_all

        @workspace.contacts.destroy_all
        @workspace.people.destroy_all
        @workspace.tags.destroy_all
        @workspace.document_types.destroy_all

        nullify_fk_landmines(@user)

        @user.destroy!
        @workspace.destroy!
      end
    end

    def delete_member!
      ApplicationRecord.transaction do
        nullify_fk_landmines(@user)
        reassign_sole_owned_accounts
        @user.destroy!
      end
    end

    # Kill the external OAuth grants before we drop the local rows, so deleting an
    # account doesn't leave Campbooks with live provider access. Best-effort: each
    # call is isolated so one failure never blocks the deletion. Runs OUTSIDE the
    # delete transaction (it does network I/O).
    def revoke_tokens_best_effort
      (@workspace.email_accounts.to_a + @workspace.calendar_accounts.to_a).each do |account|
        client = account.oauth_client
        client.revoke_token if client.respond_to?(:revoke_token)
      rescue StandardError
        # best-effort: swallow network/auth errors so deletion proceeds
      end

      revoke_google_drive_grants
      drop_notion_access
    end

    # Google Drive grants live in a separate Google project; their tokens are still
    # Google tokens, so revoke them at the same endpoint (the Drive client is
    # stateless, so pass the token in).
    def revoke_google_drive_grants
      @workspace.google_drive_accounts.find_each do |drive|
        GoogleDrive::OauthClient.new.revoke_token(drive.refresh_token)
      rescue StandardError
        # best-effort
      end
    end

    # Notion has NO token-revoke API and its bot tokens don't expire. Destroying the
    # NotionIntegration row (cascaded with the workspace) removes our copy, but the
    # user must remove the integration in Notion to fully revoke access. Log the
    # drop for the audit trail; the deletion-confirmation copy tells the user.
    def drop_notion_access
      @workspace.notion_integrations.find_each do |integration|
        Rails.logger.info(
          "[Accounts::Deleter] Dropping Notion integration #{integration.id} for workspace #{@workspace.id} " \
          "(Notion has no revoke API — local token removed only)"
        )
      end
    end

    def purge_attachments
      @workspace.documents.find_each do |doc|
        doc.original_file.purge_later if doc.original_file.attached?
        doc.processed_pdf.purge_later if doc.respond_to?(:processed_pdf) && doc.processed_pdf.attached?
      end

      @workspace.exports.find_each do |export|
        export.zip_file.purge_later if export.respond_to?(:zip_file) && export.zip_file.attached?
      end
    end

    def nullify_fk_landmines(user)
      Document.where(reviewed_by_id: user.id).update_all(reviewed_by_id: nil)
      Workflow.where(created_by_id: user.id).update_all(created_by_id: nil)
      BetaCode.where(created_by_id: user.id).update_all(created_by_id: nil)
      BetaCode.where(redeemed_by_id: user.id).update_all(redeemed_by_id: nil)
      SignupRequest.where(accepted_by_id: user.id).update_all(accepted_by_id: nil)
      SignupRequest.where(reviewed_by_id: user.id).update_all(reviewed_by_id: nil)
    end

    def reassign_sole_owned_accounts
      remaining_users = @workspace.users.where.not(id: @user.id)

      @user.email_account_users.where(owner: true).each do |eau|
        next if EmailAccountUser.where(email_account_id: eau.email_account_id, owner: true).where.not(user_id: @user.id).exists?

        new_owner = remaining_users.admin.first || remaining_users.first
        next unless new_owner

        join = EmailAccountUser.find_or_initialize_by(email_account_id: eau.email_account_id, user_id: new_owner.id)
        join.assign_attributes(owner: true, can_read: true, can_send: true, can_manage: true)
        join.save!
      end

      @user.calendar_account_users.where(owner: true).each do |cau|
        next if CalendarAccountUser.where(calendar_account_id: cau.calendar_account_id, owner: true).where.not(user_id: @user.id).exists?

        new_owner = remaining_users.admin.first || remaining_users.first
        next unless new_owner

        join = CalendarAccountUser.find_or_initialize_by(calendar_account_id: cau.calendar_account_id, user_id: new_owner.id)
        join.assign_attributes(owner: true, can_read: true, can_write: true, can_manage: true)
        join.save!
      end
    end
  end
end

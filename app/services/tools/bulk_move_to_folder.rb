module Tools
  class BulkMoveToFolder
    # Move the selected messages (and their full threads) to a folder.
    #
    # Pass `folder_id` for a single known provider id — the command-palette picker,
    # which is scoped to one message's account. Pass `folder_name` to move by name
    # (drag-to-folder / tap-to-move): each account's own provider folder is resolved
    # (created if missing) via MailFolders::Provisioner, so a cross-account custom
    # folder routes every message to the right per-account destination.
    def self.call(email_ids:, folder_id: nil, folder_name: nil)
      base = EmailMessage.accessible_to(Current.user)
      selected = base.where(id: email_ids)

      # Expand to whole threads, but keep threadless selected messages too.
      thread_ids = selected.where.not(email_thread_id: nil).distinct.pluck(:email_thread_id)
      sibling_ids = thread_ids.any? ? base.where(email_thread_id: thread_ids).pluck(:id) : []
      all_ids = (selected.pluck(:id) + sibling_ids).uniq
      messages = base.where(id: all_ids).to_a

      moved = 0
      thread_id_set = []

      messages.group_by(&:email_account_id).each do |account_id, msgs|
        account = EmailAccount.find(account_id)
        destination = folder_name.present? ? MailFolders::Provisioner.ensure_on_account(account, folder_name) : folder_id
        next if destination.blank?

        ids = msgs.map(&:id)
        EmailMessage.where(id: ids).update_all(provider_folder_id: destination, updated_at: Time.current)

        provider_ids = msgs.map(&:provider_message_id).compact
        account.mail_client.move_to_folder(provider_ids, destination) if provider_ids.any?

        moved += ids.size
        thread_id_set.concat(msgs.map(&:email_thread_id).compact)
      end

      { count: moved, folder_id: folder_id, folder_name: folder_name, thread_ids: thread_id_set.uniq }
    end
  end
end

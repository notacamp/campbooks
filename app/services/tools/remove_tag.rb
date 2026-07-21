module Tools
  class RemoveTag
    def self.call(email_message, args)
      tag_name = args["tag_name"].to_s.downcase.strip
      # Scope to the email's workspace to avoid matching a same-named tag in a
      # different workspace (cross-tenant bug).
      scope = email_message.email_account&.workspace&.tags || Tag.all
      tag = scope.find_by("LOWER(name) = ?", tag_name)
      return nil unless tag

      # Remove from every message in the thread (UI shows the thread union).
      # Local-only: this tool has never done provider sync.
      ids = email_message.email_thread ? email_message.email_thread.email_message_ids : [ email_message.id ]
      EmailMessageTag.where(email_message_id: ids, tag: tag).destroy_all
      tag
    end
  end
end

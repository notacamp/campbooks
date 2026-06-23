module Tools
  class AddTag
    def self.call(email_message, args)
      tag_name = args["tag_name"].to_s.downcase.strip
      return nil if tag_name.blank?

      # Scope to the email's workspace (consistency with the manual tag UI, which
      # only offers this workspace's tags) instead of matching any tag globally.
      scope = email_message.email_account&.workspace&.tags || Tag.all
      tag = scope.find_by("LOWER(name) = ?", tag_name)
      return nil unless tag

      email_message.email_message_tags.find_or_create_by!(tag: tag)
      Events.publish("email.tagged", subject: email_message, workspace: email_message.email_account.workspace, payload: { "subject" => email_message.subject, "tag" => tag.name })
      tag
    end
  end
end

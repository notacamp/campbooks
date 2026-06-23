module Tools
  class RemoveTag
    def self.call(email_message, args)
      tag_name = args["tag_name"].to_s.downcase.strip
      tag = Tag.find_by("LOWER(name) = ?", tag_name)
      return nil unless tag

      assignment = email_message.email_message_tags.find_by(tag: tag)
      assignment&.destroy!
      tag
    end
  end
end

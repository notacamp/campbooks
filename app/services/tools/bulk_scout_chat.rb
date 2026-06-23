module Tools
  class BulkScoutChat
    def self.call(email_ids:, user:)
      messages = EmailMessage.accessible_to(user)
                             .where(id: email_ids)
                             .includes(:email_account, :email_thread)
                             .order(received_at: :desc)

      thread = AgentThread.default_for(user)

      parts = messages.map do |msg|
        subject = msg.subject.presence || "(no subject)"
        from = msg.from_address || "unknown"
        date = msg.received_at&.strftime("%b %d, %Y at %H:%M") || "unknown date"
        summary = msg.summary.presence || msg.body.to_s.truncate(500)
        "**#{subject}**\nFrom: #{from}\nDate: #{date}\n#{summary}"
      end

      content = "I'd like you to look at these #{messages.size} email(s):\n\n#{parts.join("\n\n---\n\n")}"

      thread.agent_messages.create!(
        content: content,
        author_type: :user,
        user: user
      )

      { thread_id: thread.id, message_count: messages.size }
    end
  end
end

module Tools
  class BulkForward
    def self.call(email_ids:)
      messages = EmailMessage.where(id: email_ids)
                             .includes(:email_account, :email_thread)
                             .order(received_at: :desc)

      messages.map do |msg|
        {
          id: msg.id,
          subject: "Fwd: #{msg.subject}",
          from_address: msg.from_address,
          to_address: msg.to_address,
          received_at: msg.received_at&.strftime("%b %d, %Y at %H:%M"),
          body: msg.body
        }
      end
    end
  end
end

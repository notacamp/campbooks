module Tools
  class ForwardEmail
    def self.call(email_message, args = {})
      to_address = args[:to_address].presence
      return nil unless to_address

      account = email_message.email_account
      client = account.mail_client
      return nil unless client.respond_to?(:forward_message)

      provider_id = email_message.provider_message_id
      return nil unless provider_id

      note = args[:note].presence
      result = client.forward_message(provider_id, to_address, note: note)
      if result
        Events.publish("email.forwarded", subject: email_message, workspace: email_message.email_account.workspace, payload: { "subject" => email_message.subject, "to" => to_address })
        email_message
      end
    rescue => e
      Rails.logger.error("[Tools::ForwardEmail] Failed for message #{email_message.id}: #{e.message}")
      nil
    end
  end
end

module Tools
  class Snooze
    def self.call(email_message, args = {})
      thread = email_message.email_thread
      return unless thread

      snoozed_until = parse_snoozed_until(args["snoozed_until"])
      return unless snoozed_until

      account = email_message.email_account
      client = account.mail_client

      provider_ids = thread.email_messages.pluck(:provider_message_id).compact
      return thread if provider_ids.empty?

      snoozed_folder = client.respond_to?(:snoozed_folder_id) ? client.snoozed_folder_id : nil

      if snoozed_folder
        if client.respond_to?(:snooze_messages)
          client.snooze_messages(provider_ids)
        elsif client.respond_to?(:move_to_folder)
          client.move_to_folder(provider_ids, snoozed_folder)
        end

        thread.email_messages.update_all(
          provider_folder_id: snoozed_folder,
          updated_at: Time.current
        )
      end

      thread.update!(snoozed_until: snoozed_until)
      Events.publish("email.snoozed", subject: email_message, workspace: email_message.email_account.workspace, payload: { "subject" => email_message.subject, "until" => snoozed_until&.iso8601 })
      thread
    rescue => e
      Rails.logger.error("[Tools::Snooze] Failed for thread #{thread&.id}: #{e.message}")
      nil
    end

    def self.parse_snoozed_until(value)
      return nil if value.blank?
      time = Time.parse(value.to_s)
      time > Time.current ? time : nil
    rescue ArgumentError
      nil
    end
  end
end

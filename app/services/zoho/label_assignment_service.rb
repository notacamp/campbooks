module Zoho
  class LabelAssignmentService
    class Error < StandardError; end

    def apply(message:, tag:)
      ActiveRecord::Base.transaction do
        message.tags << tag unless message.tags.include?(tag)

        mail_client = message.email_account.mail_client
        result = mail_client.apply_labels_to_message(message.provider_message_id, [ tag.external_label_id ])

        unless result.dig("status", "code") == 200
          raise Error, "Zoho API error: #{result.dig('status', 'description') || result.inspect}"
        end
      end
    rescue ActiveRecord::RecordInvalid
      nil
    end

    def remove(message:, tag:)
      ActiveRecord::Base.transaction do
        mail_client = message.email_account.mail_client
        result = mail_client.remove_labels_from_message(message.provider_message_id, [ tag.external_label_id ])

        unless result.dig("status", "code") == 200
          raise Error, "Zoho API error: #{result.dig('status', 'description') || result.inspect}"
        end

        message.tags.delete(tag)
      end
    end
  end
end

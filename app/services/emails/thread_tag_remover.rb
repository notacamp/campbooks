module Emails
  class ThreadTagRemover
    # Removal is conversation-wide because the UI shows the thread union —
    # a per-message removal would leave the chip alive via a sibling.
    def self.call(message:, tag:)
      messages = message.email_thread ? message.email_thread.email_messages.includes(:tags).to_a : [ message ]
      messages.select! { |m| m.tags.include?(tag) }

      messages.each do |msg|
        if tag.external?
          begin
            service = msg.email_account.google? ? Google::LabelAssignmentService : Zoho::LabelAssignmentService
            service.new.remove(message: msg, tag: tag)
          rescue Zoho::LabelAssignmentService::Error, Google::LabelAssignmentService::Error => e
            Rails.logger.error("[ThreadTagRemover] Remove failed: #{e.message}")
          end
        else
          msg.tags.delete(tag)
        end
      end
    end
  end
end

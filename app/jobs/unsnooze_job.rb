class UnsnoozeJob < ApplicationJob
  queue_as :default

  retry_on StandardError, wait: :polynomially_longer, attempts: 3

  def perform
    EmailThread.expired_snoozes.find_each do |thread|
      message = thread.email_messages.order(received_at: :desc).first
      next unless message

      Tools::Unsnooze.call(message)
    rescue => e
      Rails.logger.error("[UnsnoozeJob] Failed to unsnooze thread #{thread.id}: #{e.message}")
    end
  end
end

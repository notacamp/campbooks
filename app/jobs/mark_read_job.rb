class MarkReadJob < ApplicationJob
  queue_as :default
  retry_on StandardError, wait: :polynomially_longer, attempts: 5

  def perform(email_account_id, provider_message_ids)
    account = EmailAccount.find(email_account_id)
    client = account.mail_client
    result = client.mark_read(provider_message_ids)

    if result == false || result.nil?
      raise "mark_read returned #{result.inspect} for account #{account.email_address}, ids: #{provider_message_ids.take(3)}"
    end
  end
end

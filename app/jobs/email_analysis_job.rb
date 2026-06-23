class EmailAnalysisJob < ApplicationJob
  queue_as :default
  retry_on StandardError, wait: :polynomially_longer, attempts: 3

  def perform(email_message_id)
    email = EmailMessage.find(email_message_id)
    return if email.ai_analyzed_at.present?
    return if email.tags.exists?(name: "security_flagged")

    Ai::EmailAnalyzer.new(email).analyze!

    # Scout just produced this email's summary/priority/action prompt, so the
    # home feed for everyone who can read this mailbox may have a new card.
    Feed::RefreshJob.enqueue_for_account(email.email_account)
  rescue ActiveRecord::RecordNotFound
    Rails.logger.warn("[EmailAnalysisJob] EmailMessage #{email_message_id} not found, skipping")
  rescue => e
    Rails.logger.error("[EmailAnalysisJob] Error analyzing email #{email_message_id}: #{e.message}")
    raise
  end
end

class EmailAnalysisJob < ApplicationJob
  queue_as :default
  retry_on StandardError, wait: :polynomially_longer, attempts: 3
  retry_on(*Ai::Adapters::Base::TRANSIENT_ERRORS, wait: :polynomially_longer, attempts: 5)
  limits_concurrency to: 2, key: "ai_email_analysis"

  def perform(email_message_id)
    email = EmailMessage.find(email_message_id)

    workspace = email.email_account.workspace
    Current.workspace = workspace

    return unless Ai::ProviderSetup.configured?(workspace, :text)
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
  ensure
    Current.workspace = nil
  end
end

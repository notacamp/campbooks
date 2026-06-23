class ContactAnalysisJob < ApplicationJob
  queue_as :default
  retry_on StandardError, wait: :polynomially_longer, attempts: 3

  def perform(contact_id, force: false, prompt: nil)
    contact = Contact.find(contact_id)
    return if !force && contact.analyzed_at.present? && contact.analyzed_at > 30.days.ago

    # Until a text provider is set up → don't analyse. Applies to auto and
    # user-triggered runs (this analysis is automatic, not a chat the user invoked).
    return unless Ai::ProviderSetup.configured?(contact.workspace, :text)

    Ai::ContactAnalyzer.new(contact, user_prompt: prompt).analyze!(force: force)
  rescue ActiveRecord::RecordNotFound
    Rails.logger.warn("[ContactAnalysisJob] Contact #{contact_id} not found, skipping")
  rescue => e
    Rails.logger.error("[ContactAnalysisJob] Error analyzing contact #{contact_id}: #{e.message}")
    raise
  end
end

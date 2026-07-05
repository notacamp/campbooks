class ContactAnalysisJob < ApplicationJob
  queue_as :default
  retry_on StandardError, wait: :polynomially_longer, attempts: 3

  def perform(contact_id, force: false, prompt: nil)
    contact = Contact.find(contact_id)
    return if !force && contact.analyzed_at.present? && contact.analyzed_at > 30.days.ago

    # Until a text provider is set up → don't analyse. Applies to auto and
    # user-triggered runs (this analysis is automatic, not a chat the user invoked).
    return unless Ai::ProviderSetup.configured?(contact.workspace, :text)

    # AI model resolution (Ai::Configuration.for) reads Current.workspace. Jobs run
    # outside a request, so it must be set explicitly here — without it no adapter
    # resolves, and on the cloud (where the legacy env-key fallback is disabled)
    # the analyzer silently no-ops forever: contacts never get analyzed_at, the
    # catch-up re-enqueues them every pass, and the Organizations directory stays
    # empty. Reset in `ensure` below.
    Current.workspace = contact.workspace

    Ai::ContactAnalyzer.new(contact, user_prompt: prompt).analyze!(force: force)
  rescue ActiveRecord::RecordNotFound
    Rails.logger.warn("[ContactAnalysisJob] Contact #{contact_id} not found, skipping")
  rescue => e
    Rails.logger.error("[ContactAnalysisJob] Error analyzing contact #{contact_id}: #{e.message}")
    raise
  ensure
    Current.workspace = nil
  end
end

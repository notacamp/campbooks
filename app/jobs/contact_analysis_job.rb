class ContactAnalysisJob < ApplicationJob
  queue_as :default
  retry_on StandardError, wait: :polynomially_longer, attempts: 3
  # Rate limits deserve more patience than generic errors: the analyzer lets
  # them propagate (Ai::Adapters::Base::TRANSIENT_ERRORS), and this spaces the
  # attempts out instead of losing the contact until the next catch-up pass.
  # Declared after the StandardError handler so it wins for these classes.
  # When retries are exhausted, the block increments analysis_attempts so the
  # catch-up sweep stops re-enqueuing the contact (see Contacts::PendingAnalysisCatchUp).
  retry_on(*Ai::Adapters::Base::TRANSIENT_ERRORS, wait: :polynomially_longer, attempts: 5) do |job, error|
    contact_id = job.arguments.first
    Rails.logger.warn("[ContactAnalysisJob] Retries exhausted for contact #{contact_id}: #{error.message}")
    Contact.where(id: contact_id).update_all("analysis_attempts = analysis_attempts + 1")
  end

  # The whole backlog of a freshly-connected mailbox funnels through here (the
  # catch-up enqueues up to 100 per workspace per pass). Unthrottled, that's
  # ~100 concurrent LLM calls — one burst 429'd 280/284 requests against the
  # shared managed key in prod. Two at a time keeps the drain steady and under
  # every provider's rate limit, and shields the key the rest of the app uses.
  limits_concurrency to: 2, key: "contact_analysis"

  def perform(contact_id, force: false, prompt: nil)
    contact = Contact.find(contact_id)
    return if !force && contact.analyzed_at.present? && contact.analyzed_at > 30.days.ago

    # Until a text provider is set up → don't analyse. Applies to auto and
    # user-triggered runs (this analysis is automatic, not a chat the user invoked).
    # This is not a contact failure — don't count against analysis_attempts.
    return unless Ai::ProviderSetup.configured?(contact.workspace, :text)

    # AI model resolution (Ai::Configuration.for) reads Current.workspace. Jobs run
    # outside a request, so it must be set explicitly here — without it no adapter
    # resolves, and on the cloud (where the legacy env-key fallback is disabled)
    # the analyzer silently no-ops forever: contacts never get analyzed_at, the
    # catch-up re-enqueues them every pass, and the Organizations directory stays
    # empty. Reset in `ensure` below.
    Current.workspace = contact.workspace

    Ai::ContactAnalyzer.new(contact, user_prompt: prompt).analyze!(force: force)

    contact.reload
    if contact.analyzed_at.present?
      # Success — clear the failure counter so a later re-analysis is allowed.
      contact.update_column(:analysis_attempts, 0) if contact.analysis_attempts > 0
    else
      # Analyzer returned nil (genuine failure, not a transient error or missing
      # provider — those two paths return early above or re-raise). Count it.
      Contact.where(id: contact_id).update_all("analysis_attempts = analysis_attempts + 1")
    end
  rescue ActiveRecord::RecordNotFound
    Rails.logger.warn("[ContactAnalysisJob] Contact #{contact_id} not found, skipping")
  rescue => e
    Rails.logger.error("[ContactAnalysisJob] Error analyzing contact #{contact_id}: #{e.message}")
    raise
  ensure
    Current.workspace = nil
  end
end

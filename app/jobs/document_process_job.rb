class DocumentProcessJob < ApplicationJob
  queue_as :default
  retry_on StandardError, wait: :polynomially_longer, attempts: 3

  def perform(document_id)
    document = Document.find(document_id)
    return unless document.ai_pending?

    Current.workspace = document.workspace

    # Until a document/vision provider is set up → leave the document ai_pending
    # (not failed) and analyse nothing. It can be processed once a provider is set up.
    return unless Ai::ProviderSetup.configured?(document.workspace, :documents)

    Documents::Processor.new(document).call

    # A document that finished AI analysis is a new "needs review" home-feed card.
    # Drive/Zoho auto-sync now follows human approval (Documents::FinalizeApprovalJob),
    # not processing, since nothing is auto-approved any more.
    Feed::RefreshJob.enqueue_for_workspace(document.workspace) if document.reload.review_pending? && document.ai_completed?

    # Best-effort: extract reminders (e.g. invoice due dates) from the analyzed document.
    Reminders::DocumentExtractionJob.perform_later(document.id) if document.ai_completed?

    # Best-effort: extract action items (tasks) a document implies (sign/review/return).
    # Staggered 2 minutes behind the reminders job so the novelty gate in the tasks
    # builder can see reminder rows the same document just staged (reminders-first
    # ordering feeds the cross-kind dedup gate).
    Tasks::DocumentExtractionJob.set(wait: 2.minutes).perform_later(document.id) if Features.tasks? && document.ai_completed?

    # Best-effort: Scout posts a link to the filed document into its email thread (opt-in).
    Files::ScoutThreadLinker.call(document) if document.ai_completed?
  ensure
    Current.workspace = nil
  end
end

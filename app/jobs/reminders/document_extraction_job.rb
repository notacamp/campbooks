module Reminders
  # Best-effort reminder extraction from a processed document. Runs a cheap TEXT pass
  # over the already-extracted structured data (no second vision call) and backfills
  # documents.due_date from a payment-due reminder. Enqueued from DocumentProcessJob.
  class DocumentExtractionJob < ApplicationJob
    queue_as :default
    retry_on StandardError, wait: :polynomially_longer, attempts: 3

    # Document types that never carry a forward-looking date — skip the LLM call.
    SKIP_TYPES = %w[bank_statement receipt identification bank_journal_entry].freeze

    def perform(document_id)
      document = Document.find_by(id: document_id)
      return unless document&.ai_completed?
      return if SKIP_TYPES.include?(document.document_type)

      workspace = document.workspace
      return unless Ai::ProviderSetup.configured?(workspace, :text)

      Current.workspace = workspace

      content = [
        document.description,
        document.ai_extraction_data.presence&.to_json,
        document.metadata.presence&.to_json
      ].compact_blank.join("\n")

      items = Ai::ReminderExtractor.new(
        source:      document,
        content:     content,
        anchor_date: document.document_date || document.created_at.to_date,
        time_zone:   Time.zone,
        workspace:   workspace
      ).extract

      reminders = Reminders::Builder.call(
        workspace: workspace, source: document, raw_items: items, anchor_tz: Time.zone
      )

      backfill_due_date(document, items)

      Feed::RefreshJob.enqueue_for_workspace(workspace) if reminders.any?
    ensure
      Current.workspace = nil
    end

    private

    # Populate the document's structured due_date from the first payment_due reminder,
    # so it shows in the invoice form and document views even before the user confirms.
    def backfill_due_date(document, items)
      return if document.due_date.present?

      payment = Array(items).find { |i| i["reminder_type"].to_s == "payment_due" && i["due_date"].present? }
      return unless payment

      date = (Date.iso8601(payment["due_date"]) rescue nil)
      document.update_column(:due_date, date) if date
    end
  end
end

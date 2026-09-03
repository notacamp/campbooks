# frozen_string_literal: true

# @label Paper Table
class PaperTableComponentPreview < Lookbook::Preview
  # The full facts table with one document per status.
  def default
    render Campbooks::Paper::Table.new(documents: sample_documents, folders: [])
  end

  # A single row (desktop layout).
  def row
    render Campbooks::Paper::Row.new(document: sample_documents.first, folders: [], layout: :table)
  end

  # The mobile card layout.
  def card
    render Campbooks::Paper::Row.new(document: sample_documents.first, folders: [], layout: :card)
  end

  # The kebab menu for a money document (Mark paid, exports, reprocess).
  def row_menu
    render Campbooks::Paper::RowMenu.new(document: sample_documents.first, folders: [])
  end

  private

  def sample_documents
    [
      sample(type: :expense_invoice, title: "Cloudhost · July invoice", source: :email,
             vendor_name: "Cloudhost", amount_cents: 24_800, currency: "EUR", due_date: Date.current - 20),
      sample(type: :expense_invoice, title: "Staples · order 4471", source: :email,
             vendor_name: "Staples", amount_cents: 36_400, currency: "EUR", due_date: Date.current + 5),
      sample(type: :insurance_policy, title: "Seguro Renovação 2026", review: :pending, confidence: 0.4,
             source: :email, vendor_name: "Fidelidade", period_end: Date.current + 40),
      sample(type: :contract, title: "Acme Consulting · services contract", source: :email,
             vendor_name: "Acme", period_start: Date.current - 60, period_end: Date.current.next_year),
      sample(type: :receipt, title: "Uber · ride receipt", source: :manual_upload,
             vendor_name: "Uber", amount_cents: 1_890, currency: "EUR")
    ]
  end

  def sample(type:, title:, review: :approved, confidence: 0.9, source: :email, **fields)
    doc = Document.new(id: SecureRandom.uuid, workspace_id: SecureRandom.uuid, document_type: type,
                       ai_status: :completed, review_status: review, ai_confidence_score: confidence,
                       source: source, created_at: Time.current, updated_at: Time.current)
    fields.each { |key, value| doc.public_send("#{key}=", value) }
    doc.assign_title(title)
    doc
  end
end

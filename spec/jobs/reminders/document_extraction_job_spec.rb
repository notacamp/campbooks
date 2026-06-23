require "rails_helper"

RSpec.describe Reminders::DocumentExtractionJob do
  it "skips a document that is not AI-complete" do
    doc = create(:document, ai_status: :pending)
    expect(Reminders::Builder).not_to receive(:call)
    described_class.new.perform(doc.id)
  end

  it "skips dateless document types" do
    doc = create(:document, :in_review, document_type: :bank_statement)
    expect(Ai::ReminderExtractor).not_to receive(:new)
    described_class.new.perform(doc.id)
  end

  it "extracts, builds, and backfills due_date from a payment_due item" do
    allow(Ai::ProviderSetup).to receive(:configured?).and_return(true)
    doc = create(:document, :in_review, document_type: :expense_invoice, due_date: nil)
    allow_any_instance_of(Ai::ReminderExtractor).to receive(:extract).and_return(
      [ { "reminder_type" => "payment_due", "due_date" => "2026-07-15", "title" => "Pay", "confidence" => 0.9 } ]
    )
    allow(Reminders::Builder).to receive(:call).and_return([ instance_double(Reminder) ])

    described_class.new.perform(doc.id)

    expect(doc.reload.due_date).to eq(Date.new(2026, 7, 15))
  end
end

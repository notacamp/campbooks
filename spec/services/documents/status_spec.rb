# frozen_string_literal: true

require "rails_helper"

RSpec.describe Documents::Status do
  # A fixed "today" so the boundary cases don't drift with the wall clock.
  let(:today) { Date.new(2026, 9, 3) }

  def status_for(*traits, **attrs)
    doc = create(:document, :approved, *traits, tax_amount_cents: nil, tax_rate: nil, **attrs)
    described_class.for(doc, today: today)
  end

  describe "AI lifecycle" do
    it "is processing while the AI runs" do
      result = status_for(ai_status: :processing, review_status: :pending)
      expect(result.status).to eq(:processing)
      expect(result.tone).to eq(:muted)
    end

    it "is failed (destructive) when the AI broke" do
      result = status_for(ai_status: :failed, review_status: :pending)
      expect(result.status).to eq(:failed)
      expect(result.tone).to eq(:destructive)
    end
  end

  describe "needs review" do
    it "is needs_review (ember, spark) when awaiting sign-off, ahead of the money branch" do
      result = status_for(:in_review, document_type: :expense_invoice, amount_cents: 41_200,
                          due_date: today - 5)
      expect(result.status).to eq(:needs_review)
      expect(result.tone).to eq(:ember)
      expect(result).to be_spark
    end
  end

  describe "money documents" do
    it "is unpaid (warning) when a due date is still ahead" do
      result = status_for(document_type: :expense_invoice, amount_cents: 36_400, due_date: today + 5)
      expect(result.status).to eq(:unpaid)
      expect(result.tone).to eq(:warning)
      expect(result.chip_text).to eq("Unpaid")
    end

    it "is late (warning, like the mock) with an overdue-days detail once the due date passes" do
      result = status_for(document_type: :expense_invoice, amount_cents: 24_800, due_date: today - 20)
      expect(result.status).to eq(:late)
      expect(result.tone).to eq(:warning)
      expect(result.detail).to eq("20 days")
      expect(result.chip_text).to eq("Unpaid · 20 days")
    end

    it "treats the due-date == today boundary as unpaid, not late" do
      result = status_for(document_type: :expense_invoice, amount_cents: 1_000, due_date: today)
      expect(result.status).to eq(:unpaid)
    end

    it "is paid (success) when settled — settlement wins over an overdue due date" do
      result = status_for(document_type: :expense_invoice, amount_cents: 9_640, due_date: today - 30,
                          settled_at: Time.utc(2026, 8, 28), settled_source: "bank_match")
      expect(result.status).to eq(:paid)
      expect(result.tone).to eq(:success)
      expect(result.chip_text).to eq("Paid Aug 28")
    end

    it "files a receipt that has an amount but no due date" do
      result = status_for(:receipt, amount_cents: 1_299, due_date: nil)
      expect(result.status).to eq(:filed)
    end
  end

  describe "contracts and expiring documents" do
    it "is signed (success) for a contract with no live expiry" do
      result = status_for(document_type: :contract, amount_cents: nil,
                          period_end: today + 400)
      expect(result.status).to eq(:signed)
    end

    it "is expiring (warning) within the 30-day window" do
      result = status_for(document_type: :insurance_policy, amount_cents: nil, period_end: today + 20)
      expect(result.status).to eq(:expiring)
      expect(result.tone).to eq(:warning)
    end

    it "is expired (destructive) once the end date passes" do
      result = status_for(document_type: :contract, amount_cents: nil, period_end: today - 1)
      expect(result.status).to eq(:expired)
      expect(result.tone).to eq(:destructive)
    end

    it "derives the expiry from a linked renewal reminder when there is no period_end" do
      doc = create(:document, :approved, document_type: :insurance_policy,
                   amount_cents: nil, tax_amount_cents: nil, tax_rate: nil)
      create(:reminder, source: doc, workspace: doc.workspace, reminder_type: :renewal,
             due_at: (today + 10).to_time)
      expect(described_class.for(doc, today: today).status).to eq(:expiring)
    end

    it "files a non-contract expiry type with no dates" do
      result = status_for(document_type: :certificate, amount_cents: nil, period_end: nil)
      expect(result.status).to eq(:filed)
    end
  end

  describe "bank statements" do
    it "is reconciled (success) with a ready reconciliation" do
      doc = create(:document, :approved, :bank_statement)
      create(:reconciliation, :ready, statement_document: doc, workspace: doc.workspace)
      expect(described_class.for(doc, today: today).status).to eq(:reconciled)
    end

    it "is filed without a ready reconciliation" do
      doc = create(:document, :approved, :bank_statement)
      create(:reconciliation, statement_document: doc, workspace: doc.workspace) # pending
      expect(described_class.for(doc, today: today).status).to eq(:filed)
    end
  end

  it "files everything else that is approved" do
    result = status_for(document_type: :other, amount_cents: nil)
    expect(result.status).to eq(:filed)
    expect(result.tone).to eq(:muted)
  end
end

# frozen_string_literal: true

require "rails_helper"

RSpec.describe Reconciliations::LineActions do
  let(:workspace) { create(:workspace) }
  let(:stmt) do
    create(:reconciliation, :ready, workspace: workspace,
           period_start: Date.new(2024, 1, 1), period_end: Date.new(2024, 1, 31))
  end

  def txn(**attrs)
    create(:bank_transaction, reconciliation: stmt, workspace: workspace, **attrs)
  end

  def subject_for(transaction)
    described_class.new(transaction)
  end

  describe "#confirm!" do
    it "confirms the match and marks the transaction as matched" do
      t = txn(status: :suggested)
      doc = create(:document, :approved, workspace: workspace, document_type: :expense_invoice)
      match = t.transaction_matches.create!(document: doc, status: :suggested,
                                            matched_by: :ai, confidence: 0.9,
                                            match_reasons: {})

      subject_for(t).confirm!(match.id)
      expect(t.reload.status).to eq("matched")
      expect(match.reload.status).to eq("confirmed")
    end

    it "raises ActiveRecord::RecordNotFound for a match belonging to another transaction" do
      t1 = txn(status: :suggested)
      t2 = txn(status: :suggested)
      doc = create(:document, :approved, workspace: workspace, document_type: :expense_invoice)
      match = t2.transaction_matches.create!(document: doc, status: :suggested,
                                             matched_by: :ai, confidence: 0.9,
                                             match_reasons: {})

      expect { subject_for(t1).confirm!(match.id) }.to raise_error(ActiveRecord::RecordNotFound)
    end
  end

  describe "#reject!" do
    it "marks the transaction unmatched when no other suggestions remain" do
      t = txn(status: :suggested)
      doc = create(:document, :approved, workspace: workspace, document_type: :expense_invoice)
      match = t.transaction_matches.create!(document: doc, status: :suggested,
                                            matched_by: :ai, confidence: 0.9,
                                            match_reasons: {})

      subject_for(t).reject!(match.id)
      expect(t.reload.status).to eq("unmatched")
      expect(match.reload.status).to eq("rejected")
    end

    it "keeps the transaction :suggested when other suggestions remain" do
      t = txn(status: :suggested)
      doc1 = create(:document, :approved, workspace: workspace, document_type: :expense_invoice)
      doc2 = create(:document, :approved, workspace: workspace, document_type: :expense_invoice)
      m1 = t.transaction_matches.create!(document: doc1, status: :suggested,
                                          matched_by: :ai, confidence: 0.9, match_reasons: {})
      t.transaction_matches.create!(document: doc2, status: :suggested,
                                     matched_by: :ai, confidence: 0.7, match_reasons: {})

      subject_for(t).reject!(m1.id)
      expect(t.reload.status).to eq("suggested")
    end
  end

  describe "#exclude!" do
    it "excludes the transaction with a valid reason" do
      t = txn(status: :unmatched)
      subject_for(t).exclude!("bank_fee")
      expect(t.reload.status).to eq("excluded")
      expect(t.exclusion_reason).to eq("bank_fee")
    end

    it "raises ArgumentError for an invalid reason" do
      t = txn(status: :unmatched)
      expect { subject_for(t).exclude!("nonsense") }.to raise_error(ArgumentError)
    end
  end

  describe "#reset!" do
    it "resets an excluded transaction back to unmatched" do
      t = txn(status: :excluded, exclusion_reason: "bank_fee")
      subject_for(t).reset!
      expect(t.reload.status).to eq("unmatched")
      expect(t.exclusion_reason).to be_nil
    end

    it "resets a matched transaction and rejects confirmed matches" do
      t = txn(status: :matched)
      doc = create(:document, :approved, workspace: workspace, document_type: :expense_invoice)
      match = t.transaction_matches.create!(document: doc, status: :confirmed,
                                             matched_by: :manual, confidence: 1.0,
                                             match_reasons: {})

      subject_for(t).reset!
      expect(t.reload.status).to eq("unmatched")
      expect(match.reload.status).to eq("rejected")
    end
  end

  describe "#manual_match!" do
    it "creates or upserts a confirmed match and marks the transaction matched" do
      t = txn(status: :unmatched)
      doc = create(:document, :approved, workspace: workspace, document_type: :expense_invoice)

      subject_for(t).manual_match!(doc)
      expect(t.reload.status).to eq("matched")
      expect(t.transaction_matches.confirmed.count).to eq(1)
      expect(t.transaction_matches.confirmed.first.document_id).to eq(doc.id)
    end
  end
end

# frozen_string_literal: true

require "rails_helper"

RSpec.describe Reconciliations::AutoStart do
  include ActiveJob::TestHelper

  let(:workspace) { create(:workspace, plan: "pro") }
  let!(:user)     { create(:user, workspace: workspace) }

  def statement_doc(**attrs)
    create(:document, :bank_statement, :approved, workspace: workspace, **attrs)
  end

  around { |ex| with_env("ENABLE_ACCOUNTING" => "1") { ex.run } }

  describe ".call" do
    it "starts a reconciliation for a filed bank statement and enqueues the parse" do
      doc = statement_doc(bank_name: "Millennium BCP")

      result = nil
      expect { result = described_class.call(doc) }
        .to change(Reconciliation, :count).by(1)
        .and have_enqueued_job(Reconciliations::ParseJob)

      expect(result).to be_started
      expect(result.reconciliation.statement_document).to eq(doc)
      expect(result.reconciliation.bank_name).to eq("Millennium BCP")
      expect(result.reconciliation.created_by).to eq(user)
    end

    it "credits the given user" do
      other = create(:user, workspace: workspace)
      result = described_class.call(statement_doc, created_by: other)
      expect(result.reconciliation.created_by).to eq(other)
    end

    it "says no when accounting is off" do
      doc = statement_doc
      with_env("ENABLE_ACCOUNTING" => nil) do
        expect(described_class.call(doc).reason).to eq(:accounting_off)
      end
    end

    it "says no when the workspace has no accounting entitlement" do
      free = create(:workspace, plan: "free")
      create(:user, workspace: free)
      doc = create(:document, :bank_statement, :approved, workspace: free)
      expect(described_class.call(doc).reason).to eq(:not_entitled)
    end

    it "says no for a document that isn't a bank statement" do
      doc = create(:document, :approved, workspace: workspace, document_type: :expense_invoice)
      expect(described_class.call(doc).reason).to eq(:not_a_statement)
    end

    it "says no for a rejected statement" do
      doc = create(:document, :bank_statement, :rejected, workspace: workspace)
      expect(described_class.call(doc).reason).to eq(:rejected)
    end

    it "says no when the statement is already reconciled" do
      doc = statement_doc
      create(:reconciliation, workspace: workspace, statement_document: doc, created_by: user)
      expect { expect(described_class.call(doc).reason).to eq(:already_reconciled) }
        .not_to change(Reconciliation, :count)
    end

    it "says no when the same file was already reconciled under another document" do
      twin = statement_doc(content_hash: "abc123")
      create(:reconciliation, workspace: workspace, statement_document: twin, created_by: user)
      doc = statement_doc(content_hash: "abc123")
      expect(described_class.call(doc).reason).to eq(:duplicate_file)
    end

    it "says no when a ready reconciliation already covers that bank and period" do
      create(:reconciliation, :ready, workspace: workspace, created_by: user,
             bank_name: "BPI", period_start: Date.new(2026, 7, 1), period_end: Date.new(2026, 7, 31))
      doc = statement_doc(bank_name: "BPI", period_start: Date.new(2026, 7, 1), period_end: Date.new(2026, 7, 31))
      expect(described_class.call(doc).reason).to eq(:duplicate_period)
    end

    it "starts when the period matches but the bank differs" do
      create(:reconciliation, :ready, workspace: workspace, created_by: user,
             bank_name: "BPI", period_start: Date.new(2026, 7, 1), period_end: Date.new(2026, 7, 31))
      doc = statement_doc(bank_name: "Novo Banco", period_start: Date.new(2026, 7, 1), period_end: Date.new(2026, 7, 31))
      expect(described_class.call(doc)).to be_started
    end
  end

  describe ".pending_for" do
    it "lists the workspace's unreconciled bank statements, newest first" do
      older = statement_doc(created_at: 2.days.ago)
      newer = statement_doc(created_at: 1.day.ago)
      reconciled = statement_doc
      create(:reconciliation, workspace: workspace, statement_document: reconciled, created_by: user)
      create(:document, :bank_statement, :rejected, workspace: workspace)
      create(:document, :approved, workspace: workspace, document_type: :expense_invoice)
      create(:document, :bank_statement, :approved) # another workspace

      expect(described_class.pending_for(workspace)).to eq([ newer, older ])
    end
  end
end

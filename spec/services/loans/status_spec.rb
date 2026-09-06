# frozen_string_literal: true

require "rails_helper"

RSpec.describe Loans::Status, type: :service do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }

  def make_reconciliation(period_start:, period_end:)
    doc = workspace.documents.build(document_type: :bank_statement, ai_status: :skipped,
                                    review_status: :pending, source: :manual_upload)
    doc.original_file.attach(io: StringIO.new("s"), filename: "s.csv", content_type: "text/csv")
    doc.save!
    Reconciliation.create!(workspace: workspace, created_by: user,
                           statement_document: doc, currency: "EUR",
                           status: :ready, period_start: period_start, period_end: period_end)
  end

  let!(:reconciliation) { make_reconciliation(period_start: Date.new(2024, 1, 1), period_end: Date.new(2024, 1, 31)) }

  let(:loan) do
    loan = create(:loan, workspace: workspace, created_by: user,
                         first_instalment_on: Date.new(2023, 10, 5), # before statement coverage
                         instalment_cents: 78_000, term_months: 12)
    Loans::Schedule.build!(loan)
    loan
  end

  describe ".refresh!" do
    it "marks instalments before statement coverage as unverified" do
      described_class.refresh!(loan)
      # Instalments 1-3 (Oct-Dec 2023) are before 2024-01-01
      expect(loan.instalments.where(number: 1..3).pluck(:status).uniq).to eq([ "unverified" ])
    end

    it "marks instalments within covered period with no link as missed" do
      described_class.refresh!(loan)
      # Instalment 4 is Jan 2024 — within coverage, unlinked → missed
      inst4 = loan.instalments.find_by!(number: 4)
      expect(inst4.status).to eq("missed")
    end

    it "marks instalments after coverage as expected" do
      described_class.refresh!(loan)
      # Instalments 5+ (Feb 2024+) are after 2024-01-31 + 12 days → expected
      later = loan.instalments.select { |i| i.expected_on > Date.new(2024, 2, 12) }
      expect(later.map(&:status).uniq).to eq([ "expected" ])
    end

    it "does not change paid instalments" do
      inst4 = loan.instalments.find_by!(number: 4)
      inst4.update_columns(status: LoanInstalment.statuses[:paid])
      described_class.refresh!(loan)
      expect(inst4.reload.status).to eq("paid")
    end
  end
end

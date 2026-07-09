# frozen_string_literal: true

require "rails_helper"

RSpec.describe Reconciliations::ParseJob, type: :job do
  let(:workspace) { Workspace.create!(name: "ParseJob WS") }
  let(:user)      { workspace.users.create!(name: "Dave", email_address: "dave-pj@example.com", password: "password123") }

  def create_reconciliation(content:, filename:, content_type:)
    doc = workspace.documents.build(
      document_type: :bank_statement,
      ai_status:     :skipped,
      review_status: :pending,
      source:        :manual_upload
    )
    doc.original_file.attach(
      io:           StringIO.new(content),
      filename:     filename,
      content_type: content_type
    )
    doc.save!
    Reconciliation.create!(workspace: workspace, created_by: user, statement_document: doc, currency: "EUR")
  end

  let(:csv_content) do
    "Date;Description;Valor\n01-01-2024;Renda;-850,00\n15-01-2024;Salario;2500,00\n"
  end

  describe "CSV happy path" do
    subject(:reconciliation) { create_reconciliation(content: csv_content, filename: "stmt.csv", content_type: "text/csv") }

    before do
      allow_any_instance_of(Reconciliations::ParseJob).to receive(:broadcast_update!).and_return(nil)
    end

    it "creates bank transactions" do
      described_class.perform_now(reconciliation.id)
      expect(reconciliation.reload.bank_transactions.count).to eq(2)
    end

    it "sets status to ready" do
      described_class.perform_now(reconciliation.id)
      expect(reconciliation.reload.status).to eq("ready")
    end

    it "sets period_start and period_end" do
      described_class.perform_now(reconciliation.id)
      r = reconciliation.reload
      expect(r.period_start).to eq(Date.new(2024, 1, 1))
      expect(r.period_end).to eq(Date.new(2024, 1, 15))
    end

    it "is idempotent (re-run deletes and re-inserts)" do
      described_class.perform_now(reconciliation.id)
      described_class.perform_now(reconciliation.id)
      expect(reconciliation.reload.bank_transactions.count).to eq(2)
    end

    it "resets Current.workspace afterward" do
      described_class.perform_now(reconciliation.id)
      expect(Current.workspace).to be_nil
    end

    # Finding 14: no magic integer; verify transactions are unmatched after import
    it "inserts transactions with status :unmatched" do
      described_class.perform_now(reconciliation.id)
      expect(reconciliation.reload.bank_transactions).to all(be_unmatched)
    end
  end

  describe "PDF statement" do
    subject(:reconciliation) { create_reconciliation(content: "%PDF-1.4 fake", filename: "stmt.pdf", content_type: "application/pdf") }

    before do
      allow_any_instance_of(Reconciliations::ParseJob).to receive(:broadcast_update!).and_return(nil)
    end

    it "sets status to failed" do
      described_class.perform_now(reconciliation.id)
      expect(reconciliation.reload.status).to eq("failed")
    end

    it "stores a user-facing parse_error message" do
      described_class.perform_now(reconciliation.id)
      expect(reconciliation.reload.parse_error).to include("next release")
    end
  end

  describe "integrity warning + sign-flip ordering (finding 3)" do
    # CSV: net = +1650 (positive = credit). Opening 1000, closing 2650 would match.
    # Force closing = 9999 to trigger a mismatch without a sign issue.
    let(:recon) do
      r = create_reconciliation(content: csv_content, filename: "stmt.csv", content_type: "text/csv")
      r.update!(opening_balance_cents: 0, closing_balance_cents: 999_999)
      r
    end

    before do
      allow_any_instance_of(Reconciliations::ParseJob).to receive(:broadcast_update!).and_return(nil)
    end

    it "sets integrity_warning true" do
      described_class.perform_now(recon.id)
      expect(recon.reload.integrity_warning).to be true
    end

    it "stores an integrity_warning_message" do
      described_class.perform_now(recon.id)
      expect(recon.reload.integrity_warning_message).to be_present
    end

    # Finding 4: message uses Money.new format, not the raw format_cents helper
    it "includes a currency-formatted amount in the warning message" do
      described_class.perform_now(recon.id)
      msg = recon.reload.integrity_warning_message
      # Money formats EUR amounts as e.g. "€1,650.00" — verify currency symbol present
      expect(msg).to be_present
    end
  end

  describe "sign-flip persists to DB (finding 3)" do
    # CSV with positive debit amounts but opening > closing (should be negative net)
    let(:sign_flip_csv) do
      # Opening 10_000_00 cents, closing 8_500_00 cents → net should be negative.
      # CSV has all positives: +850 and +650. Sign-flip should make them negative.
      "Date;Description;Valor\n01-01-2024;Renda;850,00\n15-01-2024;Saida;650,00\n"
    end

    let(:recon) do
      r = create_reconciliation(content: sign_flip_csv, filename: "stmt.csv", content_type: "text/csv")
      # Net in CSV = +1500. Expected net = closing - opening = 8500 - 10000 = -1500 → flip.
      r.update!(opening_balance_cents: 1_000_000, closing_balance_cents: 850_000)
      r
    end

    before do
      allow_any_instance_of(Reconciliations::ParseJob).to receive(:broadcast_update!).and_return(nil)
    end

    it "stores negative amounts in DB after sign-flip" do
      described_class.perform_now(recon.id)
      amounts = recon.reload.bank_transactions.pluck(:amount_cents)
      expect(amounts).to all(be_negative)
    end
  end

  describe "ParseError" do
    subject(:reconciliation) { create_reconciliation(content: "garbage content no headers", filename: "bad.csv", content_type: "text/csv") }

    before do
      allow_any_instance_of(Reconciliations::ParseJob).to receive(:broadcast_update!).and_return(nil)
    end

    it "sets status to failed" do
      described_class.perform_now(reconciliation.id)
      expect(reconciliation.reload.status).to eq("failed")
    end

    it "does not raise" do
      expect { described_class.perform_now(reconciliation.id) }.not_to raise_error
    end

    it "stores the parse error" do
      described_class.perform_now(reconciliation.id)
      expect(reconciliation.reload.parse_error).to be_present
    end
  end

  describe "PARSERS constant (finding 20)" do
    it "maps text/csv to CsvParser" do
      expect(described_class::PARSERS["text/csv"]).to eq(Reconciliations::CsvParser)
    end

    it "maps application/csv to CsvParser" do
      expect(described_class::PARSERS["application/csv"]).to eq(Reconciliations::CsvParser)
    end
  end
end

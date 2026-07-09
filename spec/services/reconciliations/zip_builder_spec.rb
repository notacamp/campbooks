# frozen_string_literal: true

require "rails_helper"
require "zip"

RSpec.describe Reconciliations::ZipBuilder, type: :service do
  let(:workspace) { create(:workspace, settings: { "company_nif" => "123456789" }) }
  let(:user)      { create(:user, workspace:) }

  # Build a minimal reconciliation with real ActiveStorage blobs
  let(:reconciliation) do
    doc = workspace.documents.build(
      document_type: :bank_statement,
      ai_status:     :skipped,
      review_status: :pending,
      source:        :manual_upload
    )
    doc.original_file.attach(
      io:           StringIO.new("Date,Desc,Amount\n2024-01-05,Rent,-1000.00\n"),
      filename:     "statement.csv",
      content_type: "text/csv"
    )
    doc.save!
    create(:reconciliation,
           workspace:,
           created_by:         user,
           statement_document: doc,
           currency:           "EUR",
           status:             :ready)
  end

  let(:invoice_doc) do
    d = workspace.documents.build(
      document_type: :expense_invoice,
      ai_status:     :completed,
      review_status: :approved,
      source:        :manual_upload,
      vendor_name:   "Acme Corp",
      vendor_nif:    "987654321",
      buyer_nif:     "123456789",
      invoice_number: "FT2024/001",
      amount_cents:  100000,
      currency:      "EUR",
      document_date: Date.new(2024, 1, 4)
    )
    d.original_file.attach(
      io:           StringIO.new("fake invoice content"),
      filename:     "invoice.pdf",
      content_type: "application/pdf"
    )
    d.save!
    d
  end

  let(:debit_txn) do
    create(:bank_transaction,
           reconciliation:,
           workspace:,
           position:     1,
           booked_on:    Date.new(2024, 1, 5),
           description:  "Rent payment Acme Corp",
           counterparty: "Acme Corp",
           amount_cents: -100000,
           currency:     "EUR",
           status:       :matched)
  end

  def build_zip
    described_class.new(reconciliation).call
  end

  def zip_entries(zip_data)
    buffer = StringIO.new(zip_data)
    Zip::File.open_buffer(buffer).map(&:name)
  end

  def zip_read(zip_data, entry_name)
    buffer = StringIO.new(zip_data)
    Zip::File.open_buffer(buffer).find { |e| e.name == entry_name }&.get_input_stream&.read
  end

  describe "#call" do
    context "with no transactions" do
      it "returns a valid zip with statement/ and index.csv" do
        data = build_zip
        expect(data).to be_a(String)
        entries = zip_entries(data)
        expect(entries).to include("statement/statement.csv")
        expect(entries).to include("index.csv")
      end
    end

    context "with a confirmed debit match" do
      before do
        create(:transaction_match,
               bank_transaction: debit_txn,
               document:         invoice_doc,
               status:           :confirmed,
               matched_by:       :ai,
               confidence:       0.97,
               match_reasons:    {})
      end

      it "places the invoice under debits/" do
        entries = zip_entries(build_zip)
        expect(entries.any? { |e| e.start_with?("debits/") }).to be true
      end

      it "includes the counterparty name in the filename" do
        entries = zip_entries(build_zip)
        debit_entry = entries.find { |e| e.start_with?("debits/") }
        expect(debit_entry).to include("Acme")
      end

      it "includes the invoice number in the filename" do
        entries = zip_entries(build_zip)
        debit_entry = entries.find { |e| e.start_with?("debits/") }
        expect(debit_entry).to include("FT2024")
      end

      it "writes a non-empty file inside the entry" do
        data        = build_zip
        entries     = zip_entries(data)
        debit_entry = entries.find { |e| e.start_with?("debits/") }
        content     = zip_read(data, debit_entry)
        expect(content).not_to be_empty
      end

      it "includes the transaction in index.csv with NIF status yes" do
        csv_content = zip_read(build_zip, "index.csv")
        expect(csv_content).to include("Acme Corp")
        expect(csv_content).to include("FT2024/001")
        expect(csv_content).to include("yes")
      end
    end

    context "with a matched doc missing the buyer NIF" do
      before do
        invoice_doc.update_columns(buyer_nif: nil)
        create(:transaction_match,
               bank_transaction: debit_txn,
               document:         invoice_doc,
               status:           :confirmed,
               matched_by:       :ai,
               confidence:       0.90,
               match_reasons:    {})
      end

      it "marks nif column as missing in index.csv" do
        csv_content = zip_read(build_zip, "index.csv")
        expect(csv_content).to include("missing")
      end
    end

    describe "filename sanitization" do
      it "strips forbidden characters" do
        service = described_class.new(reconciliation)
        result = service.send(:sanitize, 'bad/file*name?.pdf')
        expect(result).not_to match(%r{[/:*?"<>|]})
      end

      it "strips combining marks (accents)" do
        service = described_class.new(reconciliation)
        result = service.send(:sanitize, "Café résumé")
        expect(result).to eq("Cafe resume")
      end
    end

    describe "filename collision handling" do
      it "appends -2 for a duplicate name" do
        service = described_class.new(reconciliation)
        service.instance_variable_set(:@used_names, { "invoice.pdf" => true })
        name = service.send(:unique_name, "invoice.pdf")
        expect(name).to eq("invoice-2.pdf")
      end
    end
  end
end

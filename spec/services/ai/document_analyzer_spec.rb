require "rails_helper"

RSpec.describe Ai::DocumentAnalyzer do
  # Regression: when the model proposes a NEW document type, apply_result used to
  # call `t.organization = ...`, but DocumentType has no `organization` (it
  # `belongs_to :workspace`). Every such document crashed with
  # NoMethodError and was recorded as failed. The fix scopes creation to the
  # document's workspace and drops the bogus setter.
  describe "#call when the model proposes a new document type" do
    let(:workspace) { create(:workspace) }
    let(:document)  { create(:document, workspace: workspace, document_type: :other, ai_status: :pending) }
    let(:adapter)   { instance_double(Ai::Adapters::Openai) }

    let(:ai_response) do
      {
        document_type: "vehicle_inspection_report", # not an existing type → must be created
        title: "Inspeção IPO — 2026",
        description: "Relatório de inspeção periódica obrigatória.",
        confidence: 0.95,
        suggested_filename: "inspecao_ipo_2026",
        metadata: {}
      }.to_json
    end

    before do
      Current.workspace = workspace
      allow(Ai::Configuration).to receive(:for).with("document_analysis").and_return(
        { adapter: adapter, model: "gpt-4o-mini", max_tokens: 1000, temperature: 0.0 }
      )
      allow(adapter).to receive(:chat).and_return(ai_response)
    end

    after { Current.workspace = nil }

    it "creates the DocumentType scoped to the document's workspace without raising" do
      expect {
        described_class.new(document).call
      }.to change { workspace.document_types.where(name: "vehicle_inspection_report").count }.by(1)

      document.reload
      expect(document.ai_status).to eq("completed")
      expect(document.ai_error).to be_nil
    end

    it "does not leak the type into a different workspace" do
      other_ws = create(:workspace)
      described_class.new(document).call
      expect(other_ws.document_types.where(name: "vehicle_inspection_report")).to be_empty
    end
  end

  describe "Word (.docx) document handling" do
    let(:document) { create(:document, workspace: create(:workspace)) }
    let(:analyzer) { described_class.new(document) }
    let(:docx_ct)  { "application/vnd.openxmlformats-officedocument.wordprocessingml.document" }

    # A minimal Office Open XML package: a zip whose word/document.xml holds the body.
    def docx_bytes(text)
      Zip::OutputStream.write_buffer do |zip|
        zip.put_next_entry("word/document.xml")
        zip.write(%(<?xml version="1.0"?><w:document><w:body><w:p><w:r><w:t>#{text}</w:t></w:r></w:p></w:body></w:document>))
      end.string
    end

    it "extracts the body text from a .docx" do
      text = analyzer.send(:office_text, docx_bytes("Declaração de Aceitação da Gerência"), docx_ct)
      expect(text).to include("Declaração de Aceitação da Gerência")
    end

    it "returns nil for a non-docx content type" do
      expect(analyzer.send(:office_text, "anything", "application/pdf")).to be_nil
    end

    it "returns nil (never raises) on corrupt data" do
      expect(analyzer.send(:office_text, "not a zip", docx_ct)).to be_nil
    end

    it "sends the extracted text to the model instead of only the filename" do
      document.original_file.attach(io: StringIO.new(docx_bytes("Total a pagar: 500 EUR")),
                                    filename: "decl.docx", content_type: docx_ct)
      parts = analyzer.send(:build_generic_parts)

      expect(parts).to be_present
      expect(parts.any? { |p| p[:text].to_s.include?("Total a pagar: 500 EUR") }).to be(true)
    end
  end

  describe "apply_result metadata normalization" do
    let(:workspace) { create(:workspace) }
    let(:document)  { create(:document, workspace: workspace, document_type: :other, ai_status: :pending, metadata: nil) }
    let(:adapter)   { instance_double(Ai::Adapters::Openai) }

    before do
      Current.workspace = workspace
      allow(Ai::Configuration).to receive(:for).with("document_analysis").and_return(
        { adapter: adapter, model: "gpt-4o-mini", max_tokens: 1000, temperature: 0.0 }
      )
    end

    after { Current.workspace = nil }

    it "resolves vendor_name from insurer_name alias when vendor_name is absent" do
      allow(adapter).to receive(:chat).and_return({
        document_type: "expense_invoice",
        title: "Seguro Auto",
        confidence: 0.9,
        metadata: { "insurer_name" => "Fidelidade SA" }
      }.to_json)

      described_class.new(document).call
      document.reload
      expect(document.vendor_name).to eq("Fidelidade SA")
      expect(document.metadata["insurer_name"]).to eq("Fidelidade SA")
    end

    it "defaults currency to EUR when a money field is present and currency is absent" do
      allow(adapter).to receive(:chat).and_return({
        document_type: "expense_invoice",
        title: "Fatura",
        confidence: 0.9,
        metadata: { "vendor_name" => "Acme", "amount_cents" => 5000 }
      }.to_json)

      described_class.new(document).call
      expect(document.reload.metadata["currency"]).to eq("EUR")
    end

    it "does not override an explicit currency" do
      allow(adapter).to receive(:chat).and_return({
        document_type: "expense_invoice",
        title: "Invoice",
        confidence: 0.9,
        metadata: { "amount_cents" => 1000, "currency" => "USD" }
      }.to_json)

      described_class.new(document).call
      expect(document.reload.metadata["currency"]).to eq("USD")
    end

    it "omits a date key from metadata when the AI returns a garbage date string (schema-backed type)" do
      # Populate the expense_invoice DocumentType with its builtin schema so the
      # date field is schema-known and garbage coercion can be enforced.
      workspace.document_types.find_or_create_by!(name: "expense_invoice") do |t|
        t.color = "#aabbcc"
        t.extraction_schema = DocumentTypes::BuiltinSchemas.for("expense_invoice")
      end

      allow(adapter).to receive(:chat).and_return({
        document_type: "expense_invoice",
        title: "Fatura",
        confidence: 0.9,
        metadata: { "vendor_name" => "X", "document_date" => "not-a-date" }
      }.to_json)

      described_class.new(document).call
      expect(document.reload.metadata).not_to have_key("document_date")
    end

    it "stores a valid date as an ISO string in metadata (schema-backed type)" do
      workspace.document_types.find_or_create_by!(name: "expense_invoice") do |t|
        t.color = "#aabbcc"
        t.extraction_schema = DocumentTypes::BuiltinSchemas.for("expense_invoice")
      end

      allow(adapter).to receive(:chat).and_return({
        document_type: "expense_invoice",
        title: "Fatura",
        confidence: 0.9,
        metadata: { "vendor_name" => "X", "document_date" => "2025-03-15" }
      }.to_json)

      described_class.new(document).call
      expect(document.reload.metadata["document_date"]).to eq("2025-03-15")
    end

    it "merges normalized metadata on top of existing metadata" do
      document.update_columns(metadata: { "existing_key" => "existing_value" })

      allow(adapter).to receive(:chat).and_return({
        document_type: "expense_invoice",
        title: "Fatura",
        confidence: 0.9,
        metadata: { "vendor_name" => "New Vendor" }
      }.to_json)

      described_class.new(document).call
      document.reload
      expect(document.metadata["existing_key"]).to eq("existing_value")
      expect(document.metadata["vendor_name"]).to eq("New Vendor")
    end
  end

  # Regression: the filename heuristic's `\bata\b` word boundary (Portuguese
  # meeting-minute "ata") was corrupted into backspace control characters, which
  # silently broke the rule. Guard the boundary so unrelated names that merely
  # contain "ata" (e.g. "data") are not classified as correspondence.
  describe "#guess_from_filename — ata word boundary" do
    def guess_for(filename)
      doc = create(:document, workspace: create(:workspace))
      doc.original_file.attach(io: StringIO.new("x"), filename: filename, content_type: "application/pdf")
      described_class.new(doc).send(:guess_from_filename)
    end

    it "classifies a meeting minute (ata) as correspondence" do
      expect(guess_for("ata.pdf")).to eq("correspondence")
    end

    it "does not misclassify names that merely contain 'ata'" do
      expect(guess_for("data.pdf")).not_to eq("correspondence")
      expect(guess_for("database-export.pdf")).not_to eq("correspondence")
    end
  end
end

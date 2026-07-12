# frozen_string_literal: true

require "rails_helper"

RSpec.describe Documents::SkimBuilder do
  let(:workspace) { create(:workspace) }

  def card_for(doc)
    described_class.new([ doc ]).clusters.first
  end

  describe "#extracted_fields (the data the reviewer signs off on)" do
    context "when the document type has an extraction schema" do
      let(:schema) do
        {
          "vendor_name"    => { "type" => "string",  "description" => "Supplier" },
          "invoice_number" => { "type" => "string",  "description" => "Invoice number" },
          "amount"         => { "type" => "integer", "description" => "Total amount" },
          "notes"          => { "type" => "string" } # no description → humanized label
        }
      end
      let!(:type) do
        DocumentType.create!(workspace: workspace, name: "expense_invoice", color: "#000",
                             prompt: "t", category: "accounting", extraction_schema: schema)
      end
      let(:doc) do
        create(:document, :in_review, workspace: workspace, document_type_id: type.id,
               metadata: { "vendor_name" => "Acme Lda", "invoice_number" => "INV-9", "title" => "My invoice" })
      end

      it "surfaces every schema field, labelled by description (or humanized key)" do
        fields = card_for(doc)[:extracted_fields]
        by_key = fields.index_by { |f| f[:key] }

        # jsonb does not preserve key order, so assert membership, not sequence —
        # the card renders the same hash the detail page does.
        expect(by_key.keys).to contain_exactly("vendor_name", "invoice_number", "amount", "notes")
        expect(by_key["vendor_name"][:label]).to eq("Supplier")
        expect(by_key["invoice_number"][:label]).to eq("Invoice number")
        expect(by_key["notes"][:label]).to eq("Notes") # no description → humanized key
      end

      it "pulls each value from the document metadata" do
        fields = card_for(doc)[:extracted_fields]

        expect(fields.find { |f| f[:key] == "vendor_name" }[:value]).to eq("Acme Lda")
        expect(fields.find { |f| f[:key] == "invoice_number" }[:value]).to eq("INV-9")
      end

      it "keeps schema fields with no extracted value (the edit panel needs the full set)" do
        amount = card_for(doc)[:extracted_fields].find { |f| f[:key] == "amount" }
        expect(amount[:value]).to be_nil
      end

      it "never surfaces the display title as an extracted field" do
        keys = card_for(doc)[:extracted_fields].map { |f| f[:key] }
        expect(keys).not_to include("title")
      end
    end

    context "when a built-in type carries its canonical schema" do
      # Built-in types get their field set from the canonical BuiltinSchemas
      # extraction_schema (what the detail page renders) — not the raw metadata hash.
      # The factory's default document_type is :expense_invoice.
      let!(:type) do
        DocumentType.create!(workspace: workspace, name: "expense_invoice", color: "#000",
                             extraction_schema: DocumentTypes::BuiltinSchemas.for("expense_invoice"))
      end
      let(:doc) do
        create(:document, :in_review, workspace: workspace, vendor_name: "Acme Lda")
          .tap { |d| d.update!(metadata: (d.metadata || {}).merge("title" => "My invoice")) }
      end

      it "surfaces the built-in type's canonical schema fields, never the title" do
        keys = card_for(doc)[:extracted_fields].map { |f| f[:key] }

        expect(keys).to include("vendor_name", "invoice_number", "amount_cents", "expense_category")
        expect(keys).not_to include("title")
      end

      it "sources each value from the document metadata" do
        vendor = card_for(doc)[:extracted_fields].find { |f| f[:key] == "vendor_name" }
        expect(vendor[:value]).to eq("Acme Lda")
      end
    end

    context "when the type is free-form (no canonical columns)" do
      # A type outside the built-in set (`:contract` has no TYPE_FIELDS entry) falls
      # back to surfacing whatever the AI wrote into metadata, so nothing is hidden.
      # document_type/_id are forced post-create — the model's sync callback otherwise
      # keeps the enum at the factory default.
      let(:doc) do
        create(:document, :in_review, workspace: workspace,
               metadata: { "policy_no" => "P-1", "insurer" => "Acme", "title" => "Policy" })
          .tap { |d| d.update_columns(document_type: Document.document_types[:contract], document_type_id: nil) }
          .reload
      end

      it "falls back to the raw metadata keys (minus the title) as humanized fields" do
        fields = card_for(doc)[:extracted_fields]

        expect(fields.map { |f| f[:key] }).to contain_exactly("policy_no", "insurer")
        expect(fields.find { |f| f[:key] == "insurer" }[:label]).to eq("Insurer")
        expect(fields.find { |f| f[:key] == "policy_no" }[:value]).to eq("P-1")
      end

      it "returns an empty list when nothing was extracted (the card shows its computed summary)" do
        empty = create(:document, :in_review, workspace: workspace, metadata: {})
        empty.update_columns(document_type: Document.document_types[:contract], document_type_id: nil)
        expect(card_for(empty.reload)[:extracted_fields]).to eq([])
      end
    end

    context "when a typed-column document hides its data in metadata (boarding pass filed as 'other')" do
      # "other" has only two canonical columns (entity_name, document_date); a boarding
      # pass leaves both blank but fills metadata with flight details. The card must
      # surface that metadata rather than show an all-empty field set.
      let(:doc) do
        create(:document, :in_review, workspace: workspace, vendor_name: nil, document_date: nil,
               metadata: { "title" => "Boarding pass", "flight_number" => "TP123", "seat" => "28D" })
          .tap { |d| d.update_columns(document_type: Document.document_types[:other], document_type_id: nil) }
          .reload
      end

      it "falls back to the extracted metadata fields instead of an all-blank column set" do
        fields = card_for(doc)[:extracted_fields]

        expect(fields.map { |f| f[:key] }).to contain_exactly("flight_number", "seat")
        expect(fields.find { |f| f[:key] == "flight_number" }[:value]).to eq("TP123")
      end
    end
  end
end

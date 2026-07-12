# frozen_string_literal: true

require "rails_helper"

RSpec.describe Template, type: :model do
  describe "#apply_document_types" do
    let(:workspace) { create(:workspace) }
    let(:stale_schema) do
      { "vendor_name" => { "type" => "string", "description" => "Vendor" } }
    end
    let(:custom_schema) do
      { "policy_number" => { "type" => "string", "description" => "Policy number" } }
    end
    let(:template) do
      described_class.create!(
        name: "Test Template",
        data: {
          "document_types" => [
            { "name" => "expense_invoice", "category" => "accounting", "color" => "#3b82f6",
              "prompt" => "Supplier invoice", "extraction_schema" => stale_schema },
            { "name" => "insurance_policy", "category" => "insurance", "color" => "#a855f7",
              "prompt" => "Policy", "extraction_schema" => custom_schema }
          ]
        }
      )
    end

    it "provisions built-in types with the canonical enriched schema, not the stored one" do
      template.apply_document_types(workspace)

      schema = workspace.document_types.find_by!(name: "expense_invoice").extraction_schema
      expect(schema).to eq(DocumentTypes::BuiltinSchemas.for("expense_invoice"))
      expect(schema.dig("amount_cents", "type")).to eq("money")
      expect(schema.dig("vendor_name", "label_key")).to be_present
    end

    it "provisions non-built-in types with their stored schema" do
      template.apply_document_types(workspace)

      schema = workspace.document_types.find_by!(name: "insurance_policy").extraction_schema
      expect(schema).to eq(custom_schema)
    end

    it "does not overwrite an existing document type" do
      existing = workspace.document_types.create!(
        name: "expense_invoice", color: "#000000", extraction_schema: { "kept" => { "type" => "string" } }
      )

      template.apply_document_types(workspace)

      expect(existing.reload.extraction_schema).to eq({ "kept" => { "type" => "string" } })
    end
  end
end

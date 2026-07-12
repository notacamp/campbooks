# frozen_string_literal: true

require "rails_helper"

RSpec.describe "Documents::SkimController", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }

  before { sign_in(user) }

  describe "PATCH /documents/skim/:id/update_fields" do
    context "root-level field params (accessor path)" do
      it "saves a string field and reads it back via accessor" do
        doc = create(:document, :in_review, workspace: workspace, metadata: {})

        patch update_fields_document_skim_path(doc),
              params: { document: { vendor_name: "Novo Fornecedor" } },
              as: :json

        expect(response).to have_http_status(:ok)
        expect(doc.reload.vendor_name).to eq("Novo Fornecedor")
      end

      it "clears an enum field when blank value is submitted" do
        doc = create(:document, :in_review, workspace: workspace,
                     metadata: { "expense_category" => "travel" })

        patch update_fields_document_skim_path(doc),
              params: { document: { expense_category: "" } },
              as: :json

        expect(response).to have_http_status(:ok)
        expect(doc.reload.metadata).not_to have_key("expense_category")
      end

      it "saves description (real column, not metadata)" do
        doc = create(:document, :in_review, workspace: workspace)

        patch update_fields_document_skim_path(doc),
              params: { document: { description: "Updated desc" } },
              as: :json

        expect(doc.reload.description).to eq("Updated desc")
      end
    end

    context "nested metadata params (merge_metadata path)" do
      it "merges a field into metadata and coerces via schema" do
        # Create a document type with an expense_invoice schema so coercion is exercised.
        dt = workspace.document_types.find_or_create_by!(name: "expense_invoice") { |t|
          t.color = "#123456"
          t.extraction_schema = DocumentTypes::BuiltinSchemas.for("expense_invoice")
        }
        doc = create(:document, :in_review, workspace: workspace,
                     metadata: { "existing" => "kept" })
        doc.update_columns(document_type_id: dt.id)

        patch update_fields_document_skim_path(doc),
              params: { document: { metadata: { vendor_name: "Schema Vendor" } } },
              as: :json

        expect(response).to have_http_status(:ok)
        doc.reload
        expect(doc.vendor_name).to eq("Schema Vendor")
        expect(doc.metadata["existing"]).to eq("kept")
      end

      it "clears a metadata key when blank is submitted" do
        doc = create(:document, :in_review, workspace: workspace,
                     metadata: { "vendor_name" => "Old Vendor" })

        patch update_fields_document_skim_path(doc),
              params: { document: { metadata: { vendor_name: "" } } },
              as: :json

        expect(response).to have_http_status(:ok)
        expect(doc.reload.metadata).not_to have_key("vendor_name")
      end

      it "coerces a date field to ISO string in metadata" do
        dt = workspace.document_types.find_or_create_by!(name: "expense_invoice") { |t|
          t.color = "#123456"
          t.extraction_schema = DocumentTypes::BuiltinSchemas.for("expense_invoice")
        }
        doc = create(:document, :in_review, workspace: workspace, metadata: {})
        doc.update_columns(document_type_id: dt.id)

        patch update_fields_document_skim_path(doc),
              params: { document: { metadata: { document_date: "2025-06-15" } } },
              as: :json

        expect(doc.reload.metadata["document_date"]).to eq("2025-06-15")
        expect(doc.document_date).to eq(Date.new(2025, 6, 15))
      end

      it "does not permit arbitrary metadata keys outside the schema" do
        doc = create(:document, :in_review, workspace: workspace, metadata: {})

        patch update_fields_document_skim_path(doc),
              params: { document: { metadata: { "__proto__" => "evil", vendor_name: "ok" } } },
              as: :json

        doc.reload
        expect(doc.metadata).not_to have_key("__proto__")
        expect(doc.vendor_name).to eq("ok")
      end
    end

    it "returns display_title in the JSON response" do
      doc = create(:document, :in_review, workspace: workspace,
                   metadata: { "title" => "Old Title" })

      patch update_fields_document_skim_path(doc),
            params: { document: { title: "New Title" } },
            as: :json

      expect(response.parsed_body["display_title"]).to be_present
    end
  end
end

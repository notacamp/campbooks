require "rails_helper"

RSpec.describe "Documents", type: :request do
  before do
    @workspace = Workspace.create!(name: "Docs Redirect", slug: "docs-#{SecureRandom.hex(4)}")
    @user = @workspace.users.create!(
      name: "Docs Tester",
      email_address: "docs-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
    post session_path, params: { email_address: @user.email_address, password: "password123" }
  end

  # The Documents index merged into the Files page; the old list URL redirects.
  it "GET /documents redirects to the Files page" do
    get documents_path

    expect(response).to redirect_to(files_path)
  end

  describe "PATCH /documents/:id (update)" do
    let(:doc) do
      create(:document, workspace: @workspace,
             metadata: { "title" => "My Invoice", "vendor_name" => "Old Corp", "amount_cents" => 500 })
    end

    it "metadata merge: preserves unsubmitted keys (e.g. title set by rename)" do
      patch document_path(doc), params: {
        document: { vendor_name: "New Corp" }
      }

      doc.reload
      expect(doc.vendor_name).to eq("New Corp")
      # title must survive — it was never submitted in this form post
      expect(doc.metadata["title"]).to eq("My Invoice")
    end

    it "metadata merge: blank custom-schema value removes the key" do
      doc.update_columns(metadata: doc.metadata.merge("custom_key" => "old value"))

      patch document_path(doc), params: {
        document: { metadata: { "custom_key" => "" } }
      }

      doc.reload
      expect(doc.metadata).not_to have_key("custom_key")
      # existing keys not submitted are preserved
      expect(doc.metadata["title"]).to eq("My Invoice")
    end

    it "accessor-named params round-trip through coercion into metadata" do
      patch document_path(doc), params: {
        document: { amount_cents: "12345", vendor_name: "Acme Ltd" }
      }

      doc.reload
      expect(doc.amount_cents).to eq(12_345)
      expect(doc.vendor_name).to eq("Acme Ltd")
      expect(doc.metadata["amount_cents"]).to eq(12_345)
      expect(doc.metadata["vendor_name"]).to eq("Acme Ltd")
    end

    it "metadata merge: non-blank custom metadata key is coerced and stored" do
      # Use a document with a custom schema containing a string field
      dt = create(:document_type, workspace: @workspace, name: "Custom Type",
                  extraction_schema: { "ref_code" => { "type" => "string", "position" => 1 } })
      doc.update_columns(document_type_id: dt.id, metadata: { "title" => "Kept", "ref_code" => "OLD" })

      patch document_path(doc), params: {
        document: { metadata: { "ref_code" => "  NEW-REF  " } }
      }

      doc.reload
      expect(doc.metadata["ref_code"]).to eq("NEW-REF")   # stripped by schema coercion
      expect(doc.metadata["title"]).to eq("Kept")          # unsubmitted key preserved
    end
  end

  describe "POST /documents/perform_merge" do
    it "adopts AI data from dup into keep when keep has none; dup's values win for specific fields" do
      keep = create(:document, workspace: @workspace,
                    metadata: { "vendor_name" => "Keep Vendor", "invoice_number" => nil },
                    ai_extraction_data: nil)
      dup  = create(:document, workspace: @workspace,
                    metadata: { "vendor_name" => "Dup Vendor", "invoice_number" => "INV-99", "amount_cents" => 1000 },
                    ai_extraction_data: { "some" => "data" },
                    ai_confidence_score: 0.9)

      post perform_merge_documents_path, params: { keep_id: keep.id, merge_ids: [ dup.id ] }

      keep.reload
      # dup's vendor_name wins (dup has the AI extraction data)
      expect(keep.vendor_name).to eq("Dup Vendor")
      # dup's invoice_number adopted
      expect(keep.invoice_number).to eq("INV-99")
      # dup's amount_cents adopted
      expect(keep.amount_cents).to eq(1000)
      # dup was deleted
      expect(Document.find_by(id: dup.id)).to be_nil
    end

    it "does not overwrite keep's data when keep already has AI extraction" do
      keep = create(:document, workspace: @workspace,
                    metadata: { "vendor_name" => "Keep Vendor" },
                    ai_extraction_data: { "keep" => "data" })
      dup  = create(:document, workspace: @workspace,
                    metadata: { "vendor_name" => "Dup Vendor" },
                    ai_extraction_data: { "dup" => "data" })

      post perform_merge_documents_path, params: { keep_id: keep.id, merge_ids: [ dup.id ] }

      keep.reload
      expect(keep.vendor_name).to eq("Keep Vendor")
      expect(keep.ai_extraction_data).to eq({ "keep" => "data" })
    end
  end
end

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

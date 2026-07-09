# frozen_string_literal: true

require "rails_helper"

RSpec.describe "Reconciliations", type: :request do
  # accounting entitlement is pro+ only; use pro plan in all tests
  let(:workspace) { create(:workspace, plan: "pro") }
  let(:user)      { create(:user, workspace:) }

  around do |example|
    with_env("ENABLE_ACCOUNTING" => "1") { example.run }
  end

  describe "GET /accounting" do
    context "when not signed in" do
      it "redirects to sign-in" do
        get "/accounting"
        expect(response).to redirect_to(/session/)
      end
    end

    context "when signed in" do
      before { sign_in(user) }

      it "returns 200" do
        get "/accounting"
        expect(response).to have_http_status(:ok)
      end

      it "lists reconciliations" do
        r = create(:reconciliation, workspace:, created_by: user)
        get "/accounting"
        # The row partial links to /reconciliations/:id
        expect(response.body).to include("/reconciliations/#{r.id}")
      end
    end
  end

  describe "GET /reconciliations/new" do
    before { sign_in(user) }

    it "returns 200" do
      get "/reconciliations/new"
      expect(response).to have_http_status(:ok)
    end
  end

  describe "POST /reconciliations" do
    before { sign_in(user) }

    context "with a file upload" do
      let(:file) do
        fixture_file_upload(
          Rails.root.join("spec/fixtures/files/bank_statements/millennium_semicolon.csv"),
          "text/csv"
        )
      end

      it "creates a reconciliation and redirects to show" do
        expect {
          post "/reconciliations", params: { statement_file: file }
        }.to change(Reconciliation, :count).by(1)

        expect(response).to have_http_status(:redirect)
        expect(response.location).to include("/reconciliations/")
      end
    end

    context "with an existing document ID" do
      let!(:doc) { create(:document, :bank_statement, workspace:) }

      it "creates a reconciliation referencing the existing document" do
        expect {
          post "/reconciliations", params: { statement_document_id: doc.id }
        }.to change(Reconciliation, :count).by(1)

        expect(Reconciliation.last.statement_document_id).to eq(doc.id)
      end
    end
  end

  describe "GET /reconciliations/:id" do
    let!(:reconciliation) { create(:reconciliation, workspace:, created_by: user) }

    before { sign_in(user) }

    it "returns 200" do
      get "/reconciliations/#{reconciliation.id}"
      expect(response).to have_http_status(:ok)
    end

    it "is scoped to workspace (another workspace's record returns 404)" do
      other_ws   = create(:workspace)
      other_user = create(:user, workspace: other_ws)
      sign_in_as(other_user) # logs out the current session first

      get "/reconciliations/#{reconciliation.id}"
      expect(response).to have_http_status(:not_found)
    end
  end

  describe "DELETE /reconciliations/:id" do
    let!(:reconciliation) { create(:reconciliation, workspace:, created_by: user) }

    before { sign_in(user) }

    it "destroys the reconciliation and redirects" do
      expect {
        delete "/reconciliations/#{reconciliation.id}"
      }.to change(Reconciliation, :count).by(-1)

      expect(response).to redirect_to(accounting_path)
    end
  end

  describe "feature gate" do
    around do |example|
      with_env("ENABLE_ACCOUNTING" => nil) { example.run }
    end

    it "returns 404 when ENABLE_ACCOUNTING is not set" do
      sign_in(user)
      get "/accounting"
      expect(response).to have_http_status(:not_found)
    end
  end
end

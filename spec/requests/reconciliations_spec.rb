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
        expect(response.body).to include("/reconciliations/#{r.id}")
      end

      # Finding 7: turbo_stream format returns 200 (pagination)
      it "responds with turbo_stream for pagination" do
        get "/accounting", headers: { "Accept" => "text/vnd.turbo-stream.html" }
        expect(response).to have_http_status(:ok)
      end

      # Finding 12: /reconciliations (index route) was removed; accounting_path is canonical
      it "accounting_path helper resolves to /accounting, not /reconciliations" do
        expect(accounting_path).to eq("/accounting")
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

    # Finding 7: turbo_stream format for pagination
    it "responds with turbo_stream for pagination" do
      get "/reconciliations/#{reconciliation.id}",
          headers: { "Accept" => "text/vnd.turbo-stream.html" }
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

  # ── POST /reconciliations/:id/export ──────────────────────────────────────

  describe "POST /reconciliations/:id/export" do
    let(:reconciliation) { create(:reconciliation, :ready, workspace:, created_by: user) }

    before { sign_in(user) }

    it "enqueues ExportJob and returns turbo_stream" do
      expect(Reconciliations::ExportJob).to receive(:perform_later).with(reconciliation.id)

      post "/reconciliations/#{reconciliation.id}/export",
           headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response).to have_http_status(:ok)
    end

    it "transitions to :export_generating synchronously before enqueue" do
      allow(Reconciliations::ExportJob).to receive(:perform_later)

      post "/reconciliations/#{reconciliation.id}/export",
           headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(reconciliation.reload.export_status).to eq("export_generating")
    end

    it "returns an info message when already generating (prevents duplicate jobs)" do
      reconciliation.update!(export_status: :export_generating)

      expect(Reconciliations::ExportJob).not_to receive(:perform_later)

      post "/reconciliations/#{reconciliation.id}/export",
           headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response).to have_http_status(:ok)
    end

    it "two rapid POSTs enqueue exactly one job" do
      job_count = 0
      allow(Reconciliations::ExportJob).to receive(:perform_later) { job_count += 1 }

      post "/reconciliations/#{reconciliation.id}/export",
           headers: { "Accept" => "text/vnd.turbo-stream.html" }
      # Second POST sees :export_generating (set by the first) → blocked.
      post "/reconciliations/#{reconciliation.id}/export",
           headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(job_count).to eq(1)
    end

    it "returns 404 for a reconciliation from another workspace" do
      other_recon = create(:reconciliation, :ready,
                           workspace: create(:workspace, plan: "pro"),
                           created_by: create(:user))

      post "/reconciliations/#{other_recon.id}/export",
           headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response).to have_http_status(:not_found)
    end
  end

  # ── GET /reconciliations/:id/download ──────────────────────────────────────

  describe "GET /reconciliations/:id/download" do
    let(:reconciliation) { create(:reconciliation, :ready, workspace:, created_by: user) }

    before { sign_in(user) }

    context "when zip is not yet attached (status export_none)" do
      it "redirects back to the reconciliation page" do
        get "/reconciliations/#{reconciliation.id}/download"
        expect(response).to redirect_to(reconciliation_path(reconciliation))
      end
    end

    # Fix 3: during regeneration the old blob is still attached but the status
    # is :export_generating — the download must not serve the stale blob.
    context "when zip is attached but status is :export_generating (stale blob)" do
      before do
        reconciliation.export_zip.attach(
          io:           StringIO.new("PK\x03\x04stale zip"),
          filename:     "reconciliation-stale.zip",
          content_type: "application/zip"
        )
        reconciliation.update!(export_status: :export_generating)
      end

      it "redirects to the reconciliation page with an info flash" do
        get "/reconciliations/#{reconciliation.id}/download"
        expect(response).to redirect_to(reconciliation_path(reconciliation))
      end
    end

    context "when zip is attached and export_generated" do
      before do
        reconciliation.export_zip.attach(
          io:           StringIO.new("PK\x03\x04fake zip"),
          filename:     "reconciliation-test.zip",
          content_type: "application/zip"
        )
        reconciliation.update!(export_status: :export_generated)
      end

      it "redirects to the blob download URL" do
        get "/reconciliations/#{reconciliation.id}/download"
        expect(response).to have_http_status(:redirect)
        expect(response.location).to include("reconciliation-test.zip")
      end

      # Fix 3: billing gate on download.
      it "returns 402 for a free-plan user even with a generated zip" do
        free_user = create(:user, workspace: create(:workspace, plan: "free"))
        sign_in_as(free_user)
        free_recon = create(:reconciliation, :ready, workspace: free_user.workspace, created_by: free_user)
        free_recon.export_zip.attach(
          io: StringIO.new("PK\x03\x04zip"), filename: "r.zip", content_type: "application/zip"
        )
        free_recon.update!(export_status: :export_generated)

        get "/reconciliations/#{free_recon.id}/download",
            headers: { "Accept" => "application/json" }

        expect(response).to have_http_status(:payment_required)
      end
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

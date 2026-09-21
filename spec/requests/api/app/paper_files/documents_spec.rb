# frozen_string_literal: true

require "rails_helper"

RSpec.describe "API app Documents", type: :request do
  let(:workspace)       { create(:workspace) }
  let(:user)            { create(:user, workspace: workspace) }
  let(:other_workspace) { create(:workspace) }
  let(:other_user)      { create(:user, workspace: other_workspace) }

  let!(:document) do
    create(:document, :in_review, workspace: workspace)
  end

  describe "GET /api/app/documents/:id" do
    it "returns document detail" do
      get "/api/app/documents/#{document.id}", headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      body = response.parsed_body["data"]
      expect(body["id"]).to eq(document.id)
      expect(body["title"]).to be_present
      expect(body).to have_key("file")
      expect(body).to have_key("extraction")
    end

    it "404s for a document in another workspace" do
      other_doc = create(:document, workspace: other_workspace)

      get "/api/app/documents/#{other_doc.id}", headers: api_app_headers(user)

      expect(response).to have_http_status(:not_found)
    end

    it "requires authentication" do
      get "/api/app/documents/#{document.id}"
      expect(response).to have_http_status(:unauthorized)
    end
  end

  describe "PATCH /api/app/documents/:id/toggle_star" do
    it "toggles the starred flag" do
      original = document.starred?

      patch "/api/app/documents/#{document.id}/toggle_star", headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      body = response.parsed_body["data"]
      expect(body["starred"]).to eq(!original)
      expect(document.reload.starred?).to eq(!original)
    end

    it "404s for another workspace's document" do
      other_doc = create(:document, workspace: other_workspace)

      patch "/api/app/documents/#{other_doc.id}/toggle_star", headers: api_app_headers(user)

      expect(response).to have_http_status(:not_found)
    end
  end

  describe "POST /api/app/documents/:id/approve" do
    it "approves the document" do
      post "/api/app/documents/#{document.id}/approve", headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      body = response.parsed_body["data"]
      expect(body["review_status"]).to eq("approved")
      expect(document.reload.review_status).to eq("approved")
    end
  end

  describe "POST /api/app/documents/:id/reject" do
    it "rejects the document" do
      post "/api/app/documents/#{document.id}/reject", headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      body = response.parsed_body["data"]
      expect(body["review_status"]).to eq("rejected")
    end
  end

  describe "PATCH /api/app/documents/:id/rename" do
    it "renames the document" do
      patch "/api/app/documents/#{document.id}/rename",
            params: { document: { title: "My Invoice" } },
            headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      body = response.parsed_body["data"]
      expect(body["title"]).to eq("My Invoice")
    end
  end

  describe "POST /api/app/documents/:id/settle" do
    it "marks the document settled" do
      post "/api/app/documents/#{document.id}/settle", headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      body = response.parsed_body["data"]
      expect(body["settled"]).to be true
    end
  end

  describe "DELETE /api/app/documents/:id/settle (unsettle)" do
    before { document.mark_settled! }

    it "marks the document unsettled" do
      delete "/api/app/documents/#{document.id}/settle", headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      body = response.parsed_body["data"]
      expect(body["settled"]).to be false
    end
  end

  describe "POST /api/app/documents/:id/reprocess" do
    it "queues the document for reprocessing" do
      post "/api/app/documents/#{document.id}/reprocess", headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      body = response.parsed_body["data"]
      expect(body["ai_status"]).to eq("pending")
    end
  end

  describe "POST /api/app/documents/reprocess_all" do
    it "queues reprocessable documents" do
      post "/api/app/documents/reprocess_all", headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body.dig("data", "queued")).to be_a(Integer)
    end
  end

  describe "GET /api/app/documents/merge" do
    it "returns the specified documents for merging" do
      doc2 = create(:document, workspace: workspace)

      get "/api/app/documents/merge",
          params: { ids: "#{document.id},#{doc2.id}" },
          headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      ids = response.parsed_body["data"].map { |d| d["id"] }
      expect(ids).to include(document.id, doc2.id)
    end
  end
end

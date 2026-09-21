# frozen_string_literal: true

require "rails_helper"

RSpec.describe "API app drafts", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true, can_send: true)
  end

  def headers = api_app_headers(user)

  def make_draft(**attrs)
    DraftEmail.create!(workspace: workspace, user: user, mode: :new_message,
                       to_address: "recipient@example.com", subject: "Draft subject",
                       **attrs)
  end

  describe "GET /api/app/drafts" do
    it "returns 200 with the user's draft list" do
      make_draft
      get "/api/app/drafts", headers: headers

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body["data"]).to be_an(Array)
    end

    it "never returns another user's drafts" do
      my_draft = make_draft(subject: "Mine")
      other = create(:user, workspace: workspace)
      DraftEmail.create!(workspace: workspace, user: other, subject: "Theirs")

      get "/api/app/drafts", headers: headers
      ids = response.parsed_body["data"].map { |d| d["id"] }
      expect(ids).to include(my_draft.id)
      expect(DraftEmail.where(id: ids).pluck(:user_id).uniq).to eq([ user.id ])
    end
  end

  describe "GET /api/app/drafts/:id" do
    let(:draft) { make_draft }

    it "returns 200 with draft detail" do
      get "/api/app/drafts/#{draft.id}", headers: headers

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body["data"]["id"]).to eq(draft.id)
    end

    it "404s for another user's draft" do
      other      = create(:user, workspace: workspace)
      other_draft = DraftEmail.create!(workspace: workspace, user: other, subject: "Theirs")

      get "/api/app/drafts/#{other_draft.id}", headers: headers
      expect(response).to have_http_status(:not_found)
    end
  end

  describe "POST /api/app/drafts" do
    it "creates a draft and returns 201" do
      post "/api/app/drafts",
           params: { mode: "new_message", to: "a@example.com", subject: "New draft" },
           headers: headers

      expect(response).to have_http_status(:created)
      expect(response.parsed_body["data"]["subject"]).to eq("New draft")
    end
  end

  describe "PATCH /api/app/drafts/:id" do
    let(:draft) { make_draft(subject: "Original") }

    it "updates the draft" do
      patch "/api/app/drafts/#{draft.id}",
            params: { subject: "Updated" },
            headers: headers

      expect(response).to have_http_status(:ok)
      expect(draft.reload.subject).to eq("Updated")
    end
  end

  describe "DELETE /api/app/drafts/:id" do
    let(:draft) { make_draft }

    it "destroys the draft and returns 204" do
      delete "/api/app/drafts/#{draft.id}", headers: headers

      expect(response).to have_http_status(:no_content)
      expect(DraftEmail.find_by(id: draft.id)).to be_nil
    end
  end

  describe "POST /api/app/drafts/:id/dismiss" do
    let(:draft) { make_draft(dismissed_at: nil) }

    it "sets dismissed_at and returns the draft with dismissed: true" do
      post "/api/app/drafts/#{draft.id}/dismiss", headers: headers

      expect(response).to have_http_status(:ok)
      expect(draft.reload.dismissed_at).to be_present
      expect(response.parsed_body["data"]["dismissed"]).to be true
    end
  end

  describe "POST /api/app/drafts/:id/undismiss" do
    let(:draft) { make_draft(dismissed_at: Time.current) }

    it "clears dismissed_at" do
      post "/api/app/drafts/#{draft.id}/undismiss", headers: headers

      expect(response).to have_http_status(:ok)
      expect(draft.reload.dismissed_at).to be_nil
      expect(response.parsed_body["data"]["dismissed"]).to be false
    end
  end
end

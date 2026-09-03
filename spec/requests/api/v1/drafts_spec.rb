require "rails_helper"

RSpec.describe "API v1 drafts", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true, can_send: true)
  end

  def read_headers
    api_auth_headers(workspace: workspace, user: user, scopes: "drafts:read")
  end

  def write_headers
    api_auth_headers(workspace: workspace, user: user, scopes: "drafts:write")
  end

  def make_draft(**attrs)
    DraftEmail.create!(workspace: workspace, user: user, mode: :new_message, **attrs)
  end

  describe "GET /api/v1/drafts" do
    it "lists the acting user's drafts, newest first, with pagination meta" do
      make_draft(subject: "older", updated_at: 2.hours.ago)
      make_draft(subject: "newer", updated_at: 1.minute.ago)

      get api_v1_drafts_path, headers: read_headers

      expect(response).to have_http_status(:ok)
      body = response.parsed_body
      expect(body["data"].map { |d| d["subject"] }).to eq([ "newer", "older" ])
      expect(body["meta"]).to include("page", "total")
    end

    it "never returns another user's drafts" do
      other = create(:user, workspace: workspace)
      DraftEmail.create!(workspace: workspace, user: other, subject: "theirs")

      get api_v1_drafts_path, headers: read_headers

      expect(response.parsed_body["data"]).to be_empty
    end
  end

  describe "POST /api/v1/drafts" do
    it "creates a draft and returns it (201)" do
      post api_v1_drafts_path,
           params: { mode: "new_message", to: "a@example.com", subject: "Hi", body: "<p>yo</p>" },
           headers: write_headers

      expect(response).to have_http_status(:created)
      body = response.parsed_body["data"]
      expect(body["to"]).to eq("a@example.com")
      expect(body["subject"]).to eq("Hi")
      expect(DraftEmail.find(body["id"]).user_id).to eq(user.id)
    end

    it "links a readable reply message and a sendable account" do
      message = create(:email_message, email_account: account)

      post api_v1_drafts_path,
           params: { mode: "reply", in_reply_to_id: message.id, email_account_id: account.id, body: "re" },
           headers: write_headers

      draft = DraftEmail.find(response.parsed_body.dig("data", "id"))
      expect(draft.in_reply_to_id).to eq(message.id)
      expect(draft.email_account_id).to eq(account.id)
    end

    it "drops a foreign reply id rather than leaking it" do
      foreign = create(:email_message, email_account: create(:email_account, workspace: create(:workspace)))

      post api_v1_drafts_path,
           params: { mode: "reply", in_reply_to_id: foreign.id, body: "x" },
           headers: write_headers

      expect(response).to have_http_status(:created)
      expect(DraftEmail.find(response.parsed_body.dig("data", "id")).in_reply_to_id).to be_nil
    end
  end

  describe "PATCH /api/v1/drafts/:id" do
    it "updates content fields" do
      draft = make_draft(subject: "before")

      patch api_v1_draft_path(draft), params: { subject: "after" }, headers: write_headers

      expect(response).to have_http_status(:ok)
      expect(draft.reload.subject).to eq("after")
    end

    it "toggles the dismissed flag" do
      draft = make_draft(subject: "parked")

      patch api_v1_draft_path(draft), params: { dismissed: true }, headers: write_headers
      expect(draft.reload.dismissed_at).to be_present

      patch api_v1_draft_path(draft), params: { dismissed: false }, headers: write_headers
      expect(draft.reload.dismissed_at).to be_nil
    end

    it "404s for another user's draft" do
      other = create(:user, workspace: workspace)
      foreign = DraftEmail.create!(workspace: workspace, user: other, subject: "theirs")

      patch api_v1_draft_path(foreign), params: { subject: "hax" }, headers: write_headers

      expect(response).to have_http_status(:not_found)
    end
  end

  describe "DELETE /api/v1/drafts/:id" do
    it "destroys the draft (204)" do
      draft = make_draft(subject: "gone")

      delete api_v1_draft_path(draft), headers: write_headers

      expect(response).to have_http_status(:no_content)
      expect(DraftEmail.exists?(draft.id)).to be(false)
    end
  end

  describe "scope enforcement" do
    it "403s a write with only drafts:read" do
      post api_v1_drafts_path, params: { subject: "x" }, headers: read_headers

      expect(response).to have_http_status(:forbidden)
    end
  end
end

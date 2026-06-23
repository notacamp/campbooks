require "rails_helper"

RSpec.describe "API v1 emails", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true, can_send: true)
  end

  def read_headers
    api_auth_headers(workspace: workspace, user: user, scopes: "emails:read")
  end

  describe "GET /api/v1/emails" do
    it "lists accessible emails with pagination meta" do
      create_list(:email_message, 2, email_account: account)

      get api_v1_emails_path, headers: read_headers

      expect(response).to have_http_status(:ok)
      body = response.parsed_body
      expect(body["data"].size).to eq(2)
      expect(body["meta"]).to include("page", "per_page", "total", "total_pages")
    end

    it "excludes mail from accounts the acting user cannot read" do
      other_account = create(:email_account, workspace: workspace) # no EmailAccountUser for user
      create(:email_message, email_account: other_account)

      get api_v1_emails_path, headers: read_headers

      expect(response.parsed_body["data"]).to be_empty
    end

    it "filters by unread" do
      create(:email_message, email_account: account, read: true)
      create(:email_message, email_account: account, read: false)

      get api_v1_emails_path, params: { unread: true }, headers: read_headers

      data = response.parsed_body["data"]
      expect(data.size).to eq(1)
      expect(data.first["read"]).to be(false)
    end
  end

  describe "GET /api/v1/emails/:id" do
    it "returns the email with its body" do
      email = create(:email_message, email_account: account, body: "<p>Hi</p>")

      get api_v1_email_path(email), headers: read_headers

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body.dig("data", "body")).to eq("<p>Hi</p>")
    end

    it "404s for an email in another workspace (no existence leak)" do
      other_account = create(:email_account, workspace: create(:workspace))
      email = create(:email_message, email_account: other_account)

      get api_v1_email_path(email), headers: read_headers

      expect(response).to have_http_status(:not_found)
    end
  end

  describe "POST /api/v1/emails/:id/mark_read" do
    it "marks read and triggers a provider sync" do
      email = create(:email_message, email_account: account, read: false)
      allow(MarkReadJob).to receive(:perform_later)
      headers = api_auth_headers(workspace: workspace, user: user, scopes: "emails:write")

      post mark_read_api_v1_email_path(email), headers: headers

      expect(response).to have_http_status(:ok)
      expect(email.reload.read).to be(true)
      expect(MarkReadJob).to have_received(:perform_later).with(account.id, [ email.provider_message_id ])
    end

    it "403s with only the read scope" do
      email = create(:email_message, email_account: account)

      post mark_read_api_v1_email_path(email), headers: read_headers

      expect(response).to have_http_status(:forbidden)
    end
  end

  describe "POST /api/v1/emails (send)" do
    it "delegates to Emails::Sender and returns 201" do
      sent = create(:email_message, email_account: account)
      allow(Emails::Sender).to receive(:call).and_return(
        Emails::Sender::Result.success(email_message: sent, provider_message_id: "PMID")
      )
      headers = api_auth_headers(workspace: workspace, user: user, scopes: "emails:send")

      post api_v1_emails_path,
           params: { email_account_id: account.id, to_address: "no-reply@example.com",
                     subject: "Hi", body: "Hello" },
           headers: headers

      expect(response).to have_http_status(:created)
      expect(response.parsed_body.dig("data", "provider_message_id")).to eq("PMID")
    end

    it "surfaces a sender failure as the API error envelope" do
      allow(Emails::Sender).to receive(:call).and_return(
        Emails::Sender::Result.failure("send_failed", "boom")
      )
      headers = api_auth_headers(workspace: workspace, user: user, scopes: "emails:send")

      post api_v1_emails_path,
           params: { email_account_id: account.id, to_address: "no-reply@example.com" },
           headers: headers

      expect(response).to have_http_status(:unprocessable_entity)
      expect(response.parsed_body.dig("error", "code")).to eq("send_failed")
    end
  end
end

# frozen_string_literal: true

require "rails_helper"

RSpec.describe "API app email messages", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }
  let(:scan_log) { create(:email_scan_log, email_account: account) }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true, can_send: true, owner: true)
  end

  def headers = api_app_headers(user)

  def make_message(**attrs)
    default = { email_account: account, email_scan_log: scan_log,
                subject: "Test email", from_address: "from@example.com",
                received_at: Time.current }
    create(:email_message, **default.merge(attrs))
  end

  describe "GET /api/app/email_messages" do
    it "returns 200 with a paginated message list" do
      make_message
      get "/api/app/email_messages", headers: headers

      expect(response).to have_http_status(:ok)
      body = response.parsed_body
      expect(body["data"]).to be_an(Array)
      expect(body["meta"]).to include("page", "total")
    end

    it "401s without a token" do
      get "/api/app/email_messages"
      expect(response).to have_http_status(:unauthorized)
    end
  end

  describe "GET /api/app/email_messages/:id" do
    let(:thread) { create(:email_thread, email_account: account, subject: "Thread") }
    let(:message) { make_message(email_thread: thread) }

    it "returns 200 with message detail" do
      get "/api/app/email_messages/#{message.id}", headers: headers

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data["id"]).to eq(message.id)
      expect(data["subject"]).to eq("Test email")
      expect(data).to have_key("body")
    end

    it "marks the thread read" do
      message.update!(read: false)
      expect {
        get "/api/app/email_messages/#{message.id}", headers: headers
      }.to change { message.reload.read }.from(false).to(true)
    end

    it "404s for a message in another workspace" do
      other_workspace = create(:workspace)
      other_account   = create(:email_account, workspace: other_workspace)
      other_scan_log  = create(:email_scan_log, email_account: other_account)
      other_message   = create(:email_message, email_account: other_account, email_scan_log: other_scan_log)

      get "/api/app/email_messages/#{other_message.id}", headers: headers
      expect(response).to have_http_status(:not_found)
    end
  end

  describe "GET /api/app/email_messages/search" do
    it "returns 200 with matching messages" do
      make_message(subject: "Invoice for Q3")
      make_message(subject: "Meeting notes")

      get "/api/app/email_messages/search", params: { q: "Invoice" }, headers: headers

      expect(response).to have_http_status(:ok)
      subjects = response.parsed_body["data"].map { |m| m["subject"] }
      expect(subjects).to include("Invoice for Q3")
      expect(subjects).not_to include("Meeting notes")
    end
  end

  describe "POST /api/app/email_messages/:id/dismiss_todo" do
    let(:message) { make_message(ai_todo_dismissed: false) }

    it "marks ai_todo_dismissed true and returns 200" do
      post "/api/app/email_messages/#{message.id}/dismiss_todo", headers: headers

      expect(response).to have_http_status(:ok)
      expect(message.reload.ai_todo_dismissed).to be true
      expect(response.parsed_body["data"]["dismissed_todo"]).to be true
    end
  end
end

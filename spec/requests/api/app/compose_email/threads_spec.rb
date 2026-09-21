# frozen_string_literal: true

require "rails_helper"

RSpec.describe "API app email threads", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }
  let(:scan_log) { create(:email_scan_log, email_account: account) }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true)
  end

  def headers = api_app_headers(user)

  def make_thread_with_message
    thread = create(:email_thread, email_account: account, subject: "Thread subject")
    create(:email_message, email_account: account, email_scan_log: scan_log,
                           email_thread: thread, received_at: Time.current)
    thread
  end

  describe "GET /api/app/email_threads" do
    it "returns 200 with thread list" do
      make_thread_with_message
      get "/api/app/email_threads", headers: headers

      expect(response).to have_http_status(:ok)
      body = response.parsed_body
      expect(body["data"]).to be_an(Array)
      expect(body["meta"]).to include("page")
    end

    it "401s without a token" do
      get "/api/app/email_threads"
      expect(response).to have_http_status(:unauthorized)
    end
  end

  describe "GET /api/app/email_threads/:id" do
    let(:thread) { make_thread_with_message }

    it "returns 200 with thread detail including messages" do
      get "/api/app/email_threads/#{thread.id}", headers: headers

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data["id"]).to eq(thread.id)
      expect(data["messages"]).to be_an(Array)
    end

    it "404s for a thread in another workspace" do
      other_workspace = create(:workspace)
      other_account   = create(:email_account, workspace: other_workspace)
      other_thread    = create(:email_thread, email_account: other_account)

      get "/api/app/email_threads/#{other_thread.id}", headers: headers
      expect(response).to have_http_status(:not_found)
    end
  end
end

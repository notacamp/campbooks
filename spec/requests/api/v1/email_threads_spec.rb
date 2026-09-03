require "rails_helper"

RSpec.describe "API v1 threads", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true)
  end

  def read_headers
    api_auth_headers(workspace: workspace, user: user, scopes: "emails:read")
  end

  def write_headers
    api_auth_headers(workspace: workspace, user: user, scopes: "emails:write")
  end

  describe "GET /api/v1/threads" do
    it "lists threads from readable mailboxes with pagination meta" do
      t1 = create(:email_thread, email_account: account, subject: "Invoice")
      create(:email_message, email_account: account, email_thread: t1, subject: "Invoice", read: false)

      get api_v1_threads_path, headers: read_headers

      expect(response).to have_http_status(:ok)
      body = response.parsed_body
      expect(body["data"].first).to include("id" => t1.id, "subject" => "Invoice", "unread" => true)
      expect(body["data"].first["message_count"]).to eq(1)
      expect(body["meta"]).to include("page", "total")
    end

    it "excludes threads from mailboxes the user can't read" do
      other = create(:email_account, workspace: workspace)
      ot = create(:email_thread, email_account: other)
      create(:email_message, email_account: other, email_thread: ot)

      get api_v1_threads_path, headers: read_headers

      expect(response.parsed_body["data"]).to be_empty
    end
  end

  describe "GET /api/v1/threads/:id" do
    it "returns the thread with its messages in chronological order" do
      thread = create(:email_thread, email_account: account)
      create(:email_message, email_account: account, email_thread: thread,
             subject: "second", received_at: 1.hour.ago, body: "<p>2</p>")
      create(:email_message, email_account: account, email_thread: thread,
             subject: "first", received_at: 2.hours.ago, body: "<p>1</p>")

      get api_v1_thread_path(thread), headers: read_headers

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data["messages"].map { |m| m["subject"] }).to eq([ "first", "second" ])
      expect(data["messages"].first).to include("body" => "<p>1</p>")
      expect(data["following"]).to be(false)
    end

    it "404s a thread in another workspace (no existence leak)" do
      other = create(:email_thread, email_account: create(:email_account, workspace: create(:workspace)))

      get api_v1_thread_path(other), headers: read_headers

      expect(response).to have_http_status(:not_found)
    end
  end

  describe "follow / unfollow" do
    let(:thread) { create(:email_thread, email_account: account) }

    before { create(:email_message, email_account: account, email_thread: thread) }

    it "follows a thread, creating the backing agent thread" do
      post follow_api_v1_thread_path(thread), headers: write_headers

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body.dig("data", "following")).to be(true)
      expect(ThreadFollow.where(user: user, agent_thread: thread.reload.agent_thread)).to exist
    end

    it "is idempotent" do
      2.times { post follow_api_v1_thread_path(thread), headers: write_headers }

      expect(ThreadFollow.where(user: user, agent_thread: thread.reload.agent_thread).count).to eq(1)
    end

    it "unfollows a thread" do
      post follow_api_v1_thread_path(thread), headers: write_headers
      delete follow_api_v1_thread_path(thread), headers: write_headers

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body.dig("data", "following")).to be(false)
    end

    it "403s follow with only emails:read" do
      post follow_api_v1_thread_path(thread), headers: read_headers

      expect(response).to have_http_status(:forbidden)
    end
  end
end

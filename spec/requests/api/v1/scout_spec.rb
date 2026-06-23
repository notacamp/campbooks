require "rails_helper"

RSpec.describe "API v1 Scout", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  def read_headers
    api_auth_headers(workspace: workspace, user: user, scopes: "scout:read")
  end

  def write_headers
    api_auth_headers(workspace: workspace, user: user, scopes: "scout:write")
  end

  # Most of these don't reach the model; default to "AI is configured" so create
  # exercises the happy path. The 503 case overrides this.
  before { allow(Ai::ProviderSetup).to receive(:available?).and_return(true) }

  describe "GET /api/v1/scout/threads" do
    it "lists only the acting user's threads" do
      mine = create(:agent_thread, user: user, workspace: workspace, purpose: :global, title: "Mine")
      other_user = create(:user, workspace: workspace)
      create(:agent_thread, user: other_user, workspace: workspace, purpose: :global)

      get api_v1_scout_threads_path, headers: read_headers

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body["data"].map { |t| t["id"] }).to eq([ mine.id ])
    end

    it "403s without the scout:read scope" do
      get api_v1_scout_threads_path,
          headers: api_auth_headers(workspace: workspace, user: user, scopes: "scout:write")
      expect(response).to have_http_status(:forbidden)
    end
  end

  describe "POST /api/v1/scout/threads" do
    it "creates a global thread for the acting user" do
      expect {
        post api_v1_scout_threads_path, params: { title: "Planning" }, headers: write_headers
      }.to change { user.agent_threads.count }.by(1)

      expect(response).to have_http_status(:created)
      data = response.parsed_body["data"]
      expect(data).to include("title" => "Planning", "purpose" => "global")
    end

    it "defaults the title when none is given" do
      post api_v1_scout_threads_path, headers: write_headers
      expect(response.parsed_body.dig("data", "title")).to eq("New chat")
    end

    it "403s with only the read scope" do
      post api_v1_scout_threads_path, params: { title: "X" }, headers: read_headers
      expect(response).to have_http_status(:forbidden)
    end
  end

  describe "POST /api/v1/scout/threads/:thread_id/messages" do
    let(:thread) { create(:agent_thread, user: user, workspace: workspace, purpose: :global) }

    it "creates a user message, enqueues the reply job, and returns 202" do
      allow(AgentChatReplyJob).to receive(:perform_later)

      post api_v1_scout_thread_messages_path(thread),
           params: { content: "What needs my attention?" }, headers: write_headers

      expect(response).to have_http_status(:accepted)
      data = response.parsed_body["data"]
      expect(data).to include("author_type" => "user", "content" => "What needs my attention?")
      expect(AgentChatReplyJob).to have_received(:perform_later).with(data["id"])
    end

    it "503s when no AI provider is configured" do
      allow(Ai::ProviderSetup).to receive(:available?).and_return(false)
      allow(AgentChatReplyJob).to receive(:perform_later)

      post api_v1_scout_thread_messages_path(thread),
           params: { content: "hi" }, headers: write_headers

      expect(response).to have_http_status(:service_unavailable)
      expect(response.parsed_body.dig("error", "code")).to eq("ai_provider_unconfigured")
      expect(AgentChatReplyJob).not_to have_received(:perform_later)
    end

    it "404s for another user's thread" do
      other_user = create(:user, workspace: workspace)
      other_thread = create(:agent_thread, user: other_user, workspace: workspace, purpose: :global)

      post api_v1_scout_thread_messages_path(other_thread),
           params: { content: "hi" }, headers: write_headers

      expect(response).to have_http_status(:not_found)
    end
  end

  describe "GET /api/v1/scout/threads/:thread_id/messages" do
    let(:thread) { create(:agent_thread, user: user, workspace: workspace, purpose: :global) }

    it "returns messages chronologically and filters by after_message_id" do
      first = create(:agent_message, agent_thread: thread, user: user,
                     content: "Q", created_at: 2.minutes.ago)
      reply = create(:agent_message, agent_thread: thread, user: user, author_type: :ai,
                     content: "A", reply_status: :replied, created_at: 1.minute.ago)

      get api_v1_scout_thread_messages_path(thread), headers: read_headers
      expect(response.parsed_body["data"].map { |m| m["id"] }).to eq([ first.id, reply.id ])

      get api_v1_scout_thread_messages_path(thread, after_message_id: first.id), headers: read_headers
      data = response.parsed_body["data"]
      expect(data.map { |m| m["id"] }).to eq([ reply.id ])
      expect(data.first).to include("author_type" => "ai", "reply_status" => "replied")
    end

    it "404s for another user's thread" do
      other_user = create(:user, workspace: workspace)
      other_thread = create(:agent_thread, user: other_user, workspace: workspace, purpose: :global)

      get api_v1_scout_thread_messages_path(other_thread), headers: read_headers
      expect(response).to have_http_status(:not_found)
    end
  end
end

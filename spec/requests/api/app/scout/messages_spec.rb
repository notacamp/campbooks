# frozen_string_literal: true

require "rails_helper"

RSpec.describe "API app Scout messages", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:thread) { create(:agent_thread, user: user, workspace: workspace, purpose: :global) }

  describe "GET /api/app/scout/threads/:thread_id/messages" do
    let!(:msg1) { create(:agent_message, agent_thread: thread, user: user, content: "First") }
    let!(:msg2) { create(:agent_message, agent_thread: thread, user: user, content: "Second") }

    it "returns all messages in chronological order" do
      get "/api/app/scout/threads/#{thread.id}/messages",
          headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      contents = response.parsed_body["data"].map { |m| m["content"] }
      expect(contents).to eq([ "First", "Second" ])
    end

    it "filters by after_message_id" do
      get "/api/app/scout/threads/#{thread.id}/messages",
          params: { after_message_id: msg1.id },
          headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      contents = response.parsed_body["data"].map { |m| m["content"] }
      expect(contents).to eq([ "Second" ])
    end

    it "includes viewed_at in the payload" do
      get "/api/app/scout/threads/#{thread.id}/messages", headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body["data"].first).to have_key("viewed_at")
    end

    it "404s for another user's thread" do
      other = create(:user, workspace: workspace)
      other_thread = create(:agent_thread, user: other, workspace: workspace, purpose: :global)

      get "/api/app/scout/threads/#{other_thread.id}/messages", headers: api_app_headers(user)
      expect(response).to have_http_status(:not_found)
    end
  end

  describe "POST /api/app/scout/threads/:thread_id/messages" do
    before do
      allow(Ai::ProviderSetup).to receive(:available?).with(workspace, :text).and_return(true)
      allow(AgentChatReplyJob).to receive(:perform_later)
    end

    it "creates a user message and returns 202" do
      post "/api/app/scout/threads/#{thread.id}/messages",
           params: { content: "What is my inbox like?" },
           headers: api_app_headers(user)

      expect(response).to have_http_status(:accepted)
      data = response.parsed_body["data"]
      expect(data["author_type"]).to eq("user")
      expect(data["content"]).to eq("What is my inbox like?")
      expect(data["thread_id"]).to eq(thread.id)
    end

    it "enqueues AgentChatReplyJob" do
      post "/api/app/scout/threads/#{thread.id}/messages",
           params: { content: "Hello Scout" },
           headers: api_app_headers(user)

      expect(AgentChatReplyJob).to have_received(:perform_later).once
    end

    it "returns 503 when AI provider is not configured" do
      allow(Ai::ProviderSetup).to receive(:available?).with(workspace, :text).and_return(false)

      post "/api/app/scout/threads/#{thread.id}/messages",
           params: { content: "Hello" },
           headers: api_app_headers(user)

      expect(response).to have_http_status(:service_unavailable)
      expect(response.parsed_body.dig("error", "code")).to eq("ai_provider_unconfigured")
    end

    it "400s with missing content" do
      post "/api/app/scout/threads/#{thread.id}/messages",
           params: {},
           headers: api_app_headers(user)

      expect(response).to have_http_status(:bad_request)
    end

    it "404s for another user's thread" do
      other = create(:user, workspace: workspace)
      other_thread = create(:agent_thread, user: other, workspace: workspace, purpose: :global)

      post "/api/app/scout/threads/#{other_thread.id}/messages",
           params: { content: "Hey" },
           headers: api_app_headers(user)
      expect(response).to have_http_status(:not_found)
    end
  end
end

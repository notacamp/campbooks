# frozen_string_literal: true

require "rails_helper"

RSpec.describe "API app Scout overlay", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  describe "GET /api/app/scout/overlay" do
    before do
      allow(Ai::ProviderSetup).to receive(:available?).with(workspace, :text).and_return(true)
    end

    it "returns the idle-state overlay payload" do
      # A thread with a message so it shows up in recent_threads
      thread = create(:agent_thread, user: user, workspace: workspace, purpose: :global)
      create(:agent_message, agent_thread: thread, user: user)

      get "/api/app/scout/overlay", headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data["ai_available"]).to be(true)
      expect(data["briefing"]).to be_a(Hash)
      expect(data["briefing"]["suggestions"]).to be_an(Array)
      expect(data["recent_threads"]).to be_an(Array)
      expect(data["recent_threads"].first["id"]).to eq(thread.id)
    end

    it "reflects ai_available: false when no provider configured" do
      allow(Ai::ProviderSetup).to receive(:available?).with(workspace, :text).and_return(false)

      get "/api/app/scout/overlay", headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body.dig("data", "ai_available")).to be(false)
    end

    it "excludes setup_chat threads from recent_threads" do
      setup = create(:agent_thread, user: user, workspace: workspace, purpose: :setup_chat)
      create(:agent_message, agent_thread: setup, user: user)

      get "/api/app/scout/overlay", headers: api_app_headers(user)

      ids = response.parsed_body.dig("data", "recent_threads").map { |t| t["id"] }
      expect(ids).not_to include(setup.id)
    end

    it "401s without a token" do
      get "/api/app/scout/overlay"
      expect(response).to have_http_status(:unauthorized)
    end
  end
end

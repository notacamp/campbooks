# frozen_string_literal: true

require "rails_helper"

RSpec.describe "API app Scout threads", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  describe "GET /api/app/scout/threads" do
    it "returns paginated visible threads that have messages" do
      thread = create(:agent_thread, user: user, workspace: workspace, purpose: :global)
      create(:agent_message, agent_thread: thread, user: user)
      # setup_chat thread should be excluded
      setup = create(:agent_thread, user: user, workspace: workspace, purpose: :setup_chat)
      create(:agent_message, agent_thread: setup, user: user)

      get "/api/app/scout/threads", headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      ids = response.parsed_body["data"].map { |t| t["id"] }
      expect(ids).to include(thread.id)
      expect(ids).not_to include(setup.id)
    end

    it "401s without a token" do
      get "/api/app/scout/threads"
      expect(response).to have_http_status(:unauthorized)
    end

    it "does not leak another user's threads" do
      other = create(:user, workspace: workspace)
      other_thread = create(:agent_thread, user: other, workspace: workspace, purpose: :global)
      create(:agent_message, agent_thread: other_thread, user: other)

      get "/api/app/scout/threads", headers: api_app_headers(user)

      ids = response.parsed_body["data"].map { |t| t["id"] }
      expect(ids).not_to include(other_thread.id)
    end
  end

  describe "POST /api/app/scout/threads" do
    it "creates a new global thread" do
      post "/api/app/scout/threads",
           params: { title: "My thread" },
           headers: api_app_headers(user)

      expect(response).to have_http_status(:created)
      body = response.parsed_body["data"]
      expect(body["title"]).to eq("My thread")
      expect(body["id"]).to be_present
    end

    it "uses 'New chat' as default title" do
      post "/api/app/scout/threads", headers: api_app_headers(user)

      expect(response).to have_http_status(:created)
      expect(response.parsed_body.dig("data", "title")).to eq("New chat")
    end
  end

  describe "GET /api/app/scout/threads/:id" do
    let(:thread) { create(:agent_thread, user: user, workspace: workspace, purpose: :global) }
    let!(:msg) { create(:agent_message, agent_thread: thread, user: user, content: "Hello") }

    it "returns thread with messages" do
      get "/api/app/scout/threads/#{thread.id}", headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data["id"]).to eq(thread.id)
      expect(data["messages"].length).to eq(1)
      expect(data["messages"].first["content"]).to eq("Hello")
    end

    it "404s for another user's thread" do
      other = create(:user, workspace: workspace)
      other_thread = create(:agent_thread, user: other, workspace: workspace, purpose: :global)

      get "/api/app/scout/threads/#{other_thread.id}", headers: api_app_headers(user)
      expect(response).to have_http_status(:not_found)
    end

    it "404s for a setup_chat thread" do
      setup = create(:agent_thread, user: user, workspace: workspace, purpose: :setup_chat)

      get "/api/app/scout/threads/#{setup.id}", headers: api_app_headers(user)
      expect(response).to have_http_status(:not_found)
    end
  end

  describe "PATCH /api/app/scout/threads/:id" do
    let(:thread) { create(:agent_thread, user: user, workspace: workspace, purpose: :global) }

    it "renames the thread" do
      patch "/api/app/scout/threads/#{thread.id}",
            params: { title: "Renamed" },
            headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body.dig("data", "title")).to eq("Renamed")
      expect(thread.reload.title).to eq("Renamed")
    end

    it "404s for another user's thread" do
      other = create(:user, workspace: workspace)
      other_thread = create(:agent_thread, user: other, workspace: workspace)

      patch "/api/app/scout/threads/#{other_thread.id}",
            params: { title: "x" },
            headers: api_app_headers(user)
      expect(response).to have_http_status(:not_found)
    end
  end

  describe "DELETE /api/app/scout/threads/:id" do
    let!(:thread) { create(:agent_thread, user: user, workspace: workspace, purpose: :global) }

    it "deletes the thread" do
      delete "/api/app/scout/threads/#{thread.id}", headers: api_app_headers(user)

      expect(response).to have_http_status(:no_content)
      expect(AgentThread.find_by(id: thread.id)).to be_nil
    end

    it "404s for another user's thread" do
      other = create(:user, workspace: workspace)
      other_thread = create(:agent_thread, user: other, workspace: workspace)

      delete "/api/app/scout/threads/#{other_thread.id}", headers: api_app_headers(user)
      expect(response).to have_http_status(:not_found)
    end
  end
end

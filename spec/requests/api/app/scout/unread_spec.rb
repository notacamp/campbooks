# frozen_string_literal: true

require "rails_helper"

RSpec.describe "API app Scout unread + mark_read", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:thread) { create(:agent_thread, user: user, workspace: workspace, purpose: :global) }

  describe "GET /api/app/scout/unread" do
    it "returns count of unviewed AI messages" do
      create(:agent_message, agent_thread: thread, user: user,
             author_type: :ai, viewed_at: nil)
      create(:agent_message, agent_thread: thread, user: user,
             author_type: :ai, viewed_at: Time.current)  # already viewed
      create(:agent_message, agent_thread: thread, user: user,
             author_type: :user, viewed_at: nil)           # user message, not counted

      get "/api/app/scout/unread", headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body.dig("data", "count")).to eq(1)
    end

    it "does not count messages from another user's threads" do
      other = create(:user, workspace: workspace)
      other_thread = create(:agent_thread, user: other, workspace: workspace, purpose: :global)
      create(:agent_message, agent_thread: other_thread, user: other,
             author_type: :ai, viewed_at: nil)

      get "/api/app/scout/unread", headers: api_app_headers(user)

      expect(response.parsed_body.dig("data", "count")).to eq(0)
    end

    it "401s without a token" do
      get "/api/app/scout/unread"
      expect(response).to have_http_status(:unauthorized)
    end
  end

  describe "POST /api/app/scout/mark_read" do
    it "marks all unviewed AI messages as viewed" do
      msg = create(:agent_message, agent_thread: thread, user: user,
                   author_type: :ai, viewed_at: nil)

      post "/api/app/scout/mark_read", headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body.dig("data", "marked_read")).to eq(1)
      expect(msg.reload.viewed_at).not_to be_nil
    end

    it "does not touch messages from another user's threads" do
      other = create(:user, workspace: workspace)
      other_thread = create(:agent_thread, user: other, workspace: workspace, purpose: :global)
      other_msg = create(:agent_message, agent_thread: other_thread, user: other,
                         author_type: :ai, viewed_at: nil)

      post "/api/app/scout/mark_read", headers: api_app_headers(user)

      expect(other_msg.reload.viewed_at).to be_nil
    end

    it "401s without a token" do
      post "/api/app/scout/mark_read"
      expect(response).to have_http_status(:unauthorized)
    end
  end
end

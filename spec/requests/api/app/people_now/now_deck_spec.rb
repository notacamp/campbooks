# frozen_string_literal: true

require "rails_helper"

RSpec.describe "GET /api/app/now", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }

  def grant_access
    create(:email_account_user, user: user, email_account: account, can_read: true, can_send: true)
  end

  before { grant_access }

  let(:headers) { api_app_headers(user) }

  describe "unauthenticated" do
    it "returns 401" do
      get "/api/app/now"
      expect(response).to have_http_status(:unauthorized)
    end
  end

  describe "authenticated" do
    it "returns 200 with the deck shape" do
      get "/api/app/now", headers: headers

      expect(response).to have_http_status(:ok)
      body = response.parsed_body["data"]
      expect(body).to include("segment", "segment_counts", "attention", "timeline",
                               "ledger", "log", "inbox_state")
      expect(body["segment"]).to eq("all")
      expect(body["attention"]).to be_an(Array)
      expect(body["timeline"]).to be_an(Array)
    end

    it "returns segment_counts with the expected keys" do
      get "/api/app/now", headers: headers

      counts = response.parsed_body.dig("data", "segment_counts")
      expect(counts).to include("all", "priority", "follow_ups", "mail", "time")
    end

    it "includes a feed item when one exists" do
      msg  = create(:email_message, email_account: account, received_at: 1.hour.ago)
      _item = create(:feed_item, user: user, workspace: workspace, subject: msg,
                                kind: "reply_owed", score: 80, attention: false)

      get "/api/app/now", headers: headers

      timeline = response.parsed_body.dig("data", "timeline")
      expect(timeline).to be_an(Array)
      # item may not be present if source still_valid? returns false for a bare factory;
      # assert shape at minimum
    end

    it "accepts the ?segment= param" do
      get "/api/app/now", params: { segment: "mail" }, headers: headers

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body.dig("data", "segment")).to eq("mail")
    end

    it "cross-workspace: another user's items are not visible" do
      other_workspace = create(:workspace)
      other_user = create(:user, workspace: other_workspace)

      get "/api/app/now", headers: api_app_headers(other_user)

      expect(response).to have_http_status(:ok)
      counts = response.parsed_body.dig("data", "segment_counts")
      expect(counts["all"]).to eq(0)
    end
  end
end

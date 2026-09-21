# frozen_string_literal: true

require "rails_helper"

RSpec.describe "Event Types API (app)", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }

  around do |ex|
    travel_to ::Time.zone.local(2026, 9, 20, 12, 0, 0), &ex
  end

  def build_event_type(ws, name)
    ws.event_types.create!(name: name, icon: "")
  end

  # ── GET /api/app/event_types ──────────────────────────────────────────────────

  describe "GET /api/app/event_types" do
    it "returns all event types for the workspace" do
      et1 = build_event_type(workspace, "Meeting")
      et2 = build_event_type(workspace, "Deadline")

      get "/api/app/event_types", headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      ids = response.parsed_body["data"].map { |t| t["id"] }
      expect(ids).to include(et1.id, et2.id)
    end

    it "does not return event types from another workspace" do
      other_ws = create(:workspace)
      other_ws.event_types.create!(name: "Other", icon: "")
      build_event_type(workspace, "Mine")

      get "/api/app/event_types", headers: api_app_headers(user)

      names = response.parsed_body["data"].map { |t| t["name"] }
      expect(names).to include("Mine")
      expect(names).not_to include("Other")
    end

    it "401s without a token" do
      get "/api/app/event_types"
      expect(response).to have_http_status(:unauthorized)
    end
  end

  # ── POST /api/app/event_types ─────────────────────────────────────────────────

  describe "POST /api/app/event_types" do
    it "creates a new event type" do
      post "/api/app/event_types",
           params: { name: "Focus time", icon: "" },
           headers: api_app_headers(user)

      expect(response).to have_http_status(:created)
      data = response.parsed_body["data"]
      expect(data["name"]).to eq("Focus time")
    end

    it "422s with a duplicate name" do
      build_event_type(workspace, "Meeting")

      post "/api/app/event_types",
           params: { name: "Meeting" },
           headers: api_app_headers(user)

      expect(response).to have_http_status(:unprocessable_entity)
    end
  end

  # ── PATCH /api/app/event_types/:id ────────────────────────────────────────────

  describe "PATCH /api/app/event_types/:id" do
    let(:event_type) { build_event_type(workspace, "Travel") }

    it "updates the event type" do
      patch "/api/app/event_types/#{event_type.id}",
            params: { name: "Trip" },
            headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body.dig("data", "name")).to eq("Trip")
    end

    it "404s for an event type in another workspace" do
      other_ws = create(:workspace)
      other_et = other_ws.event_types.create!(name: "Theirs", icon: "")

      patch "/api/app/event_types/#{other_et.id}",
            params: { name: "Mine" },
            headers: api_app_headers(user)

      expect(response).to have_http_status(:not_found)
    end
  end

  # ── DELETE /api/app/event_types/:id ───────────────────────────────────────────

  describe "DELETE /api/app/event_types/:id" do
    let(:event_type) { build_event_type(workspace, "Old") }

    it "deletes the event type" do
      delete "/api/app/event_types/#{event_type.id}", headers: api_app_headers(user)

      expect(response).to have_http_status(:no_content)
      expect(EventType.exists?(event_type.id)).to be(false)
    end
  end

  # ── POST /api/app/event_types/starters ────────────────────────────────────────

  describe "POST /api/app/event_types/starters" do
    it "seeds the starter set and returns all types" do
      post "/api/app/event_types/starters", headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      names = response.parsed_body["data"].map { |t| t["name"] }
      starter_names = EventType::STARTERS.map { |s| s[:name] }
      expect(names).to include(*starter_names)
    end

    it "is idempotent — re-running does not duplicate names" do
      post "/api/app/event_types/starters", headers: api_app_headers(user)
      post "/api/app/event_types/starters", headers: api_app_headers(user)

      expect(workspace.event_types.count).to eq(EventType::STARTERS.size)
    end
  end
end

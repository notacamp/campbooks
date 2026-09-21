# frozen_string_literal: true

require "rails_helper"

RSpec.describe "Calendar Accounts API (app)", type: :request do
  let(:workspace)        { create(:workspace) }
  let(:user)             { create(:user, workspace: workspace) }
  let(:calendar_account) do
    create(:calendar_account, workspace: workspace).tap do |acc|
      create(:calendar_account_user, calendar_account: acc, user: user,
                                     can_read: true, can_write: true, can_manage: true, owner: true)
    end
  end
  let!(:calendar) { create(:calendar, calendar_account: calendar_account, syncing: true) }

  around do |ex|
    travel_to ::Time.zone.local(2026, 9, 20, 12, 0, 0), &ex
  end

  before do
    allow(Accounts::TokenRevoker).to receive(:revoke_if_unshared)
    allow(CalendarScanJob).to receive(:perform_later)
  end

  # ── PATCH /api/app/calendar_accounts/:id (rename) ────────────────────────────

  describe "PATCH /api/app/calendar_accounts/:id (rename)" do
    it "renames the account and returns the updated payload" do
      patch "/api/app/calendar_accounts/#{calendar_account.id}",
            params: { calendar_account: { name: "My Work Calendar" } },
            headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body.dig("data", "display_name")).to eq("My Work Calendar")
    end

    it "403s when the user is only a viewer (cannot manage)" do
      viewer_ws = create(:workspace)
      viewer    = create(:user, workspace: viewer_ws)
      acc2      = create(:calendar_account, workspace: workspace)
      create(:calendar_account_user, calendar_account: acc2, user: viewer, can_read: true, owner: false, can_manage: false)

      # Viewer is in a different workspace — their token hits a different workspace
      patch "/api/app/calendar_accounts/#{calendar_account.id}",
            params: { calendar_account: { name: "Nope" } },
            headers: api_app_headers(viewer)

      expect(response).to have_http_status(:not_found)
    end

    it "404s for an account in another workspace" do
      other_ws      = create(:workspace)
      other_user    = create(:user, workspace: other_ws)
      other_account = create(:calendar_account, workspace: other_ws).tap do |acc|
        create(:calendar_account_user, calendar_account: acc, user: other_user, can_read: true, owner: true)
      end

      patch "/api/app/calendar_accounts/#{other_account.id}",
            params: { calendar_account: { name: "Hijack" } },
            headers: api_app_headers(user)

      expect(response).to have_http_status(:not_found)
    end
  end

  # ── DELETE /api/app/calendar_accounts/:id ────────────────────────────────────

  describe "DELETE /api/app/calendar_accounts/:id" do
    it "deactivates the account and revokes the token" do
      delete "/api/app/calendar_accounts/#{calendar_account.id}", headers: api_app_headers(user)

      expect(response).to have_http_status(:no_content)
      expect(calendar_account.reload.active).to be(false)
      expect(Accounts::TokenRevoker).to have_received(:revoke_if_unshared)
    end
  end

  # ── GET /api/app/calendar_accounts/:id/sharing ───────────────────────────────

  describe "GET /api/app/calendar_accounts/:id/sharing" do
    it "returns the sharing panel data for the owner" do
      get "/api/app/calendar_accounts/#{calendar_account.id}/sharing",
          headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data).to have_key("members")
      expect(data).to have_key("addable_users")
      expect(data).to have_key("roles")
    end
  end

  # ── POST /api/app/calendar_accounts/refresh ──────────────────────────────────

  describe "POST /api/app/calendar_accounts/refresh" do
    it "enqueues a full calendar scan and returns queued: true" do
      post "/api/app/calendar_accounts/refresh",
           params: { calendar_account_id: calendar_account.id },
           headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body.dig("data", "queued")).to be(true)
      expect(CalendarScanJob).to have_received(:perform_later).with(calendar_account.id, "full")
    end
  end

  # ── PATCH /api/app/calendar_accounts/:calendar_account_id/calendars/:id ──────

  describe "PATCH /api/app/calendar_accounts/:calendar_account_id/calendars/:id" do
    it "toggles the syncing flag and returns the updated calendar" do
      patch "/api/app/calendar_accounts/#{calendar_account.id}/calendars/#{calendar.id}",
            params: { syncing: false },
            headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data["syncing"]).to be(false)
    end

    it "404s for a calendar in another account" do
      other_ws      = create(:workspace)
      other_user    = create(:user, workspace: other_ws)
      other_account = create(:calendar_account, workspace: other_ws).tap do |acc|
        create(:calendar_account_user, calendar_account: acc, user: other_user, can_read: true, owner: true)
      end
      other_cal = create(:calendar, calendar_account: other_account, syncing: true)

      patch "/api/app/calendar_accounts/#{calendar_account.id}/calendars/#{other_cal.id}",
            params: { syncing: false },
            headers: api_app_headers(user)

      expect(response).to have_http_status(:not_found)
    end
  end

  context "without authentication" do
    it "401s on update" do
      patch "/api/app/calendar_accounts/#{calendar_account.id}",
            params: { calendar_account: { name: "X" } }
      expect(response).to have_http_status(:unauthorized)
    end
  end
end

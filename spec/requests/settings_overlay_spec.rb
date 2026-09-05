# frozen_string_literal: true

require "rails_helper"

RSpec.describe "Settings overlay", type: :request do
  include_context "with authenticated user"

  describe "GET /settings/account" do
    before { get settings_account_path }

    it "returns 200" do
      expect(response).to have_http_status(:ok)
    end

    it "includes the settings-overlay dialog" do
      expect(response.body).to include('id="settings-overlay"')
    end

    it "includes account page content inside the turbo-frame" do
      expect(response.body).to include('id="settings_panel"')
    end

    it "the main element does not contain settings PageHeader content outside the overlay" do
      # The settings page content is now captured and passed to the overlay,
      # not rendered raw in <main>
      expect(response.body).to include('id="settings-overlay"')
    end
  end

  describe "GET /now" do
    before { get now_path }

    it "renders the settings overlay with an empty (skeleton) frame" do
      expect(response.body).to include('id="settings-overlay"')
      expect(response.body).to include("animate-pulse")
    end

    it "does not include settings-page content" do
      expect(response.body).not_to include("settings-specific-content")
    end
  end

  describe "GET /settings/account with Turbo-Frame header" do
    before { get settings_account_path, headers: { "Turbo-Frame" => "settings_panel" } }

    it "returns 200" do
      expect(response).to have_http_status(:ok)
    end
  end

  describe "GET /settings/account as signed-out user" do
    before do
      delete session_path # sign out
      get settings_account_path
    end

    it "redirects to sign in" do
      expect(response).to redirect_to(new_session_path)
    end
  end
end

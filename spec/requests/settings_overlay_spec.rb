# frozen_string_literal: true

require "rails_helper"

# Settings pages render inside the global settings overlay: the layout captures
# the page and hands it to Campbooks::SettingsOverlay, whose Turbo frame
# (#settings_panel) carries it; <main> stays empty. Non-settings pages render the
# overlay closed with a skeleton in the frame.
RSpec.describe "Settings overlay", type: :request do
  let(:ws) { Workspace.create!(name: "Overlay WS", slug: "overlay-#{SecureRandom.hex(4)}") }
  let(:user) do
    ws.users.create!(name: "Overlay Tester", email_address: "overlay-#{SecureRandom.hex(4)}@example.com", password: "password123")
  end

  before { sign_in(user) }

  describe "GET /settings/account" do
    before { get settings_account_path }

    it "renders the account page inside the overlay's frame, not in <main>" do
      expect(response).to have_http_status(:ok)
      body = response.body
      expect(body).to include('id="settings-overlay"')
      frame_start = body.index('<turbo-frame id="settings_panel"')
      frame_end   = body.index("</turbo-frame>", frame_start)
      expect(frame_start).not_to be_nil
      expect(body[frame_start..frame_end]).to include(I18n.t("settings.account.show.title"))

      main_start = body.index('<main id="main-content"')
      main_end   = body.index("</main>", main_start)
      expect(body[main_start..main_end]).not_to include(I18n.t("settings.account.show.title"))
    end

    it "marks the frame with the page URL so the client does not refetch it, and highlights the nav item" do
      expect(response.body).to include('data-current-url="http://www.example.com/settings/account"')
      expect(response.body).to include('aria-current="page"')
      expect(response.body).to include(I18n.t("settings.catalog.groups.you"))
    end
  end

  describe "GET /now" do
    before { get now_path }

    it "renders the overlay closed with a skeleton frame and no settings content" do
      expect(response).to have_http_status(:ok)
      expect(response.body).to include('id="settings-overlay"')
      expect(response.body).to include("animate-pulse")
      expect(response.body).not_to include('data-current-url=')
      expect(response.body).not_to include(I18n.t("settings.account.show.title"))
    end
  end

  describe "GET /settings/inbox/rules as a Turbo-Frame request" do
    before { get settings_inbox_section_path("rules"), headers: { "Turbo-Frame" => "settings_panel" } }

    it "renders only the overlay's frame (no shell, no dialog) so the pane swaps in place" do
      expect(response).to have_http_status(:ok)
      expect(response.body).to include('<turbo-frame id="settings_panel"')
      expect(response.body).to include('data-current-url="http://www.example.com/settings/inbox/rules"')
      expect(response.body).to include("inbox_settings_panel")
      expect(response.body).to include(I18n.t("settings.catalog.groups.inbox"))
      expect(response.body).not_to include('id="settings-overlay"')
      expect(response.body).not_to include('<main')
    end
  end

  describe "GET /settings/account signed out" do
    it "redirects to sign in" do
      delete session_path
      get settings_account_path
      expect(response).to redirect_to(new_session_path)
    end
  end
end

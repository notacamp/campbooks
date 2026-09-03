# frozen_string_literal: true

require "rails_helper"

# Nav labels with apostrophes or & are HTML-escaped in ERB:
#   "Scout's memory"      → "Scout&#39;s memory"
#   "AI & automation"     → "AI &amp; automation"
#   "Workspace & people"  → "Workspace &amp; people"
# We assert on safe fragments or on CGI.escapeHTML of the i18n string.

RSpec.describe "Settings memory nav", type: :request do
  let(:ws) { Workspace.create!(name: "Nav WS", slug: "nav-#{SecureRandom.hex(4)}") }
  let(:user) do
    ws.users.create!(
      name: "Nav Tester",
      email_address: "nav-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
  end

  before { sign_in(user) }

  # ---------------------------------------------------------------------------
  # Bold nav: flag on + user layout_bold
  # ---------------------------------------------------------------------------
  describe "bold-layout nav" do
    before do
      allow(Features).to receive(:bold_layout?).and_return(true)
      user.update!(layout_mode: :bold)
    end

    it "shows all five bold nav labels on the memory page" do
      get settings_memory_path
      expect(response).to have_http_status(:ok)

      # Labels with special chars are HTML-escaped by ERB (apostrophe → &#39;,
      # & → &amp;), so we match on the escaped versions.
      expect(response.body).to include("Scout&#39;s memory")      # Scout's memory
      expect(response.body).to include("Connections")
      expect(response.body).to include("Workspace &amp; people")  # Workspace & people
      expect(response.body).to include("Account")
      expect(response.body).to include("Plan")
    end

    it "does not show the classic 'AI & automation' group heading" do
      get settings_memory_path
      expect(response).to have_http_status(:ok)
      # Both escaped and unescaped variants absent
      expect(response.body).not_to include("AI &amp; automation")
      expect(response.body).not_to include("AI &amp; Automation")
    end
  end

  # ---------------------------------------------------------------------------
  # Classic nav: flag off (default) — always shows classic headings
  # ---------------------------------------------------------------------------
  describe "classic nav — flag off" do
    it "shows the classic Inbox group heading on the account settings page" do
      get settings_account_path
      expect(response).to have_http_status(:ok)
      # "Inbox" has no special chars
      expect(response.body).to include(I18n.t("navigation.settings.groups.inbox"))
    end

    it "shows the classic AI & automation group heading on the account settings page" do
      get settings_account_path
      expect(response).to have_http_status(:ok)
      # "AI & automation" → "AI &amp; automation" in HTML (the & is escaped)
      expect(response.body).to include("AI &amp; automation")
    end
  end

  # ---------------------------------------------------------------------------
  # Classic nav: flag on but user is classic layout
  # ---------------------------------------------------------------------------
  describe "classic nav — flag on, user classic layout" do
    before { allow(Features).to receive(:bold_layout?).and_return(true) }

    it "shows classic AI & automation heading when user stays on classic layout" do
      # user.layout_mode defaults to :classic — ApplicationController#bold_layout?
      # requires BOTH Features.bold_layout? AND current_user.layout_bold?, so the
      # classic user still gets the classic nav even when the flag is on.
      expect(user.layout_classic?).to be true

      get settings_account_path
      expect(response).to have_http_status(:ok)
      expect(response.body).to include("AI &amp; automation")
    end

    it "does not show the five-place bold label 'Workspace & people' when user is classic" do
      get settings_account_path
      expect(response).to have_http_status(:ok)
      expect(response.body).not_to include("Workspace &amp; people")
    end
  end
end

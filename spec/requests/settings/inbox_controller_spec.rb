require "rails_helper"

RSpec.describe "Settings::InboxController", type: :request do
  let(:ws) { Workspace.create!(name: "Inbox Ctrl WS", slug: "inbox-ctrl-#{SecureRandom.hex(4)}") }
  let(:user) do
    ws.users.create!(
      name: "Inbox Tester",
      email_address: "inbox-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
  end

  it "a section page requires authentication" do
    get settings_inbox_section_path("tags")
    expect(response).to have_http_status(:found)
  end

  context "when signed in" do
    before { sign_in(user) }

    it "bare /settings/inbox redirects to the first panel" do
      get settings_inbox_path
      expect(response).to redirect_to(settings_inbox_section_path("tags"))
    end

    it "a section page embeds the matching panel via the shared frame" do
      get settings_inbox_section_path("filtering")
      expect(response).to have_http_status(:ok)
      expect(response.body).to include("inbox_settings_panel")
      expect(response.body).to include(inbox_settings_filtering_path)
    end

    it "an unknown section 404s" do
      get settings_inbox_section_path("bogus")
      expect(response).to have_http_status(:not_found)
    end

    it "the settings sidebar has an Inbox group with every panel, active item highlighted" do
      get settings_inbox_section_path("tags")
      expect(response).to have_http_status(:ok)
      # One sidebar item per catalog section, so a panel can't be silently dropped.
      InboxSettings::Sections::ALL.each do |section|
        expect(response.body).to include(settings_inbox_section_path(section[:key]))
      end
      # The current section is the highlighted one (only the sidebar marks active state).
      expect(response.body).to include(settings_inbox_section_path("tags"))
      expect(response.body).to include('aria-current="page"')
    end
  end
end

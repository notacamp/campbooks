# frozen_string_literal: true

require "rails_helper"

# The settings overlay now shows six catalog groups: You, Scout, Inbox, Paper,
# Connections, Workspace. The old five-group sidebar ("Workspace & people", "Account",
# "Plan", "AI & automation") is gone.
#
# Nav labels with apostrophes are HTML-escaped in ERB:
#   "Scout's memory" → "Scout&#39;s memory"

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

  it "shows all six overlay nav group labels on the memory page" do
    get settings_memory_path
    expect(response).to have_http_status(:ok)

    # The new overlay nav groups:
    expect(response.body).to include("You")
    expect(response.body).to include("Scout")
    expect(response.body).to include("Inbox")
    expect(response.body).to include("Paper")
    expect(response.body).to include("Connections")
    expect(response.body).to include("Workspace")
  end

  it "includes Scout's memory as a nav item" do
    get settings_memory_path
    expect(response).to have_http_status(:ok)
    # Apostrophe is HTML-escaped in ERB output
    expect(response.body).to include("Scout&#39;s memory")
  end

  it "does not show the old sidebar group headings" do
    get settings_memory_path
    expect(response).to have_http_status(:ok)
    expect(response.body).not_to include("Workspace &amp; people")
    expect(response.body).not_to include("AI &amp; automation")
    expect(response.body).not_to include("AI &amp; Automation")
  end
end

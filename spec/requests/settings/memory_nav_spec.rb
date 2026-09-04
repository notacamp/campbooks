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

  it "shows all five nav labels on the memory page" do
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
    expect(response.body).not_to include("AI &amp; automation")
    expect(response.body).not_to include("AI &amp; Automation")
  end
end

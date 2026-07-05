require "rails_helper"

# The native iOS/Android shell (Hotwire Native) hides desktop-only / keyboard-only
# surfaces. turbo-rails identifies it by "(Turbo|Hotwire) Native" in the UA.
RSpec.describe "Native feature gating", type: :request do
  NATIVE_UA = "Campbooks/1.0 iOS Hotwire Native".freeze

  before do
    @workspace = Workspace.create!(name: "Gate Test", slug: "gate-#{SecureRandom.hex(4)}")
    @user = @workspace.users.create!(
      name: "Gate Tester",
      email_address: "gate-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
    post session_path, params: { email_address: @user.email_address, password: "password123" }
  end

  it "calendar offers week and month views on the web" do
    get calendar_path

    expect(response).to have_http_status(:ok)
    doc = Nokogiri::HTML(response.body)
    expect(doc.css("a[data-calendar-view='week']")).not_to be_empty
    expect(doc.css("a[data-calendar-view='month']")).not_to be_empty
  end

  it "calendar hides week and month views in the native app" do
    get calendar_path, headers: { "HTTP_USER_AGENT" => NATIVE_UA }

    expect(response).to have_http_status(:ok)
    doc = Nokogiri::HTML(response.body)
    expect(doc.css("a[data-calendar-view='agenda']")).not_to be_empty
    expect(doc.css("a[data-calendar-view='day']")).not_to be_empty
    expect(doc.css("a[data-calendar-view='week']")).to be_empty
    expect(doc.css("a[data-calendar-view='month']")).to be_empty
  end

  it "calendar coerces a week/month deep link back to agenda in the native app" do
    get calendar_path(view: "month"), headers: { "HTTP_USER_AGENT" => NATIVE_UA }

    expect(response).to have_http_status(:ok)
    # The active tab carries the `bg-card` highlight; month was coerced to agenda.
    doc = Nokogiri::HTML(response.body)
    expect(doc.css("a.bg-card[data-calendar-view='agenda']")).not_to be_empty
  end

  it "settings links to API Access on the web but not in the native app" do
    get settings_root_path
    doc = Nokogiri::HTML(response.body)
    expect(doc.css("a[href='#{settings_api_clients_path}']")).not_to be_empty

    get settings_root_path, headers: { "HTTP_USER_AGENT" => NATIVE_UA }
    doc = Nokogiri::HTML(response.body)
    expect(doc.css("a[href='#{settings_api_clients_path}']")).to be_empty
  end
end

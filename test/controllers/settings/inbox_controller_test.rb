require "test_helper"

class Settings::InboxControllerTest < ActionDispatch::IntegrationTest
  setup do
    @ws = Workspace.create!(name: "Inbox Ctrl WS", slug: "inbox-ctrl-#{SecureRandom.hex(4)}")
    @user = @ws.users.create!(
      name: "Inbox Tester",
      email_address: "inbox-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
  end

  test "a section page requires authentication" do
    get settings_inbox_section_path("tags")
    assert_response :redirect
  end

  test "bare /settings/inbox redirects to the first panel" do
    sign_in
    get settings_inbox_path
    assert_redirected_to settings_inbox_section_path("tags")
  end

  test "a section page embeds the matching panel via the shared frame" do
    sign_in
    get settings_inbox_section_path("filtering")
    assert_response :success
    assert_includes @response.body, "inbox_settings_panel"
    assert_includes @response.body, inbox_settings_filtering_path
  end

  test "an unknown section 404s" do
    sign_in
    get settings_inbox_section_path("bogus")
    assert_response :not_found
  end

  test "the settings sidebar has an Inbox group with every panel, active item highlighted" do
    sign_in
    get settings_inbox_section_path("tags")
    assert_response :success
    # One sidebar item per catalog section, so a panel can't be silently dropped.
    InboxSettings::Sections::ALL.each do |section|
      assert_select "a[href=?]", settings_inbox_section_path(section[:key])
    end
    # The current section is the highlighted one (only the sidebar marks active state).
    assert_select "a[href=?][aria-current=?]", settings_inbox_section_path("tags"), "page"
  end

  private

  def sign_in
    post session_path, params: { email_address: @user.email_address, password: "password123" }
  end
end

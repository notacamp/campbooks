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

  test "requires authentication" do
    get settings_inbox_path
    assert_response :redirect
  end

  test "show renders the shared panel frame, eager-loads Tags, and links every section" do
    sign_in
    get settings_inbox_path
    assert_response :success

    # The Turbo Frame the InboxSettings::* panels render into (same id as the modal).
    assert_includes @response.body, "inbox_settings_panel"
    # The frame eager-loads the default (Tags) panel.
    assert_includes @response.body, inbox_settings_tags_path
    # A sub-nav entry for every catalog section, so the page can't silently drop one.
    InboxSettings::Sections::ALL.each do |section|
      assert_includes @response.body, %(data-section="#{section[:key]}"),
        "expected a sub-nav link for the #{section[:key]} panel"
    end
  end

  test "highlights the Inbox item in the settings sidebar" do
    sign_in
    get settings_inbox_path
    assert_response :success
    # Only the settings sidebar links to settings_inbox_path, and only it marks the
    # active section (the topbar user menu renders the same groups without aria-current).
    assert_select "a[href=?][aria-current=?]", settings_inbox_path, "page"
  end

  private

  def sign_in
    post session_path, params: { email_address: @user.email_address, password: "password123" }
  end
end

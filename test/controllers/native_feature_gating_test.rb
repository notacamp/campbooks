require "test_helper"

# The native iOS/Android shell (Hotwire Native) hides desktop-only / keyboard-only
# surfaces. turbo-rails identifies it by "(Turbo|Hotwire) Native" in the UA.
class NativeFeatureGatingTest < ActionDispatch::IntegrationTest
  NATIVE_UA = "Campbooks/1.0 iOS Hotwire Native".freeze

  setup do
    @workspace = Workspace.create!(name: "Gate Test", slug: "gate-#{SecureRandom.hex(4)}")
    @user = @workspace.users.create!(
      name: "Gate Tester",
      email_address: "gate-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
    post session_path, params: { email_address: @user.email_address, password: "password123" }
  end

  test "calendar offers week and month views on the web" do
    get calendar_path

    assert_response :success
    assert_select "a[data-calendar-view='week']"
    assert_select "a[data-calendar-view='month']"
  end

  test "calendar hides week and month views in the native app" do
    get calendar_path, headers: { "HTTP_USER_AGENT" => NATIVE_UA }

    assert_response :success
    assert_select "a[data-calendar-view='agenda']"
    assert_select "a[data-calendar-view='day']"
    assert_select "a[data-calendar-view='week']", count: 0
    assert_select "a[data-calendar-view='month']", count: 0
  end

  test "calendar coerces a week/month deep link back to agenda in the native app" do
    get calendar_path(view: "month"), headers: { "HTTP_USER_AGENT" => NATIVE_UA }

    assert_response :success
    # The active tab carries the `bg-card` highlight; month was coerced to agenda.
    assert_select "a[data-calendar-view='agenda'].bg-card"
  end

  test "settings links to API Access on the web but not in the native app" do
    get settings_root_path
    assert_select "a[href=?]", settings_api_clients_path

    get settings_root_path, headers: { "HTTP_USER_AGENT" => NATIVE_UA }
    assert_select "a[href=?]", settings_api_clients_path, count: 0
  end
end

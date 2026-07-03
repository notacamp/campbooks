require "test_helper"

class InboxSettings::SmartGroupsControllerTest < ActionDispatch::IntegrationTest
  setup do
    @workspace = Workspace.create!(name: "SG Settings WS")
    @user = @workspace.users.create!(
      name: "Ana", email_address: "ana-#{SecureRandom.hex(4)}@example.com", password: "password123"
    )
    sign_in(@user)
  end

  def sign_in(user)
    post session_path, params: { email_address: user.email_address, password: "password123" }
  end

  test "requires authentication" do
    delete session_path
    get inbox_settings_smart_groups_path

    assert_redirected_to new_session_path
  end

  test "show renders the panel" do
    get inbox_settings_smart_groups_path

    assert_response :success
    assert_match "smart_groups_panel", response.body
  end

  test "update persists prefs and ignores unknown keys" do
    patch inbox_settings_smart_groups_path, params: {
      smart_groups: { enabled: "1", promotions: "0", social: "1", bogus: "1" }
    }

    prefs = @user.reload.inbox_smart_groups
    assert_equal true, prefs["enabled"]
    assert_equal false, prefs["promotions"]
    assert_equal true, prefs["social"]
    assert_nil prefs["bogus"]
    assert_not @user.smart_group_enabled?("promotions")
    assert @user.smart_group_enabled?("notifications")
  end

  test "update can disable the whole feature" do
    patch inbox_settings_smart_groups_path, params: { smart_groups: { enabled: "0" } }

    assert_not @user.reload.smart_groups_enabled?
    assert_equal [], @user.enabled_smart_group_buckets
  end
end

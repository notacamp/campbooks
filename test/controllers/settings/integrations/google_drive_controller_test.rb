require "test_helper"

class Settings::Integrations::GoogleDriveControllerTest < ActionDispatch::IntegrationTest
  setup do
    @ws = Workspace.create!(name: "GDrive Show WS", slug: "gd-show-#{SecureRandom.hex(4)}")
    @user = @ws.users.create!(
      name: "GDrive Show Tester",
      email_address: "gds-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
    post session_path, params: { email_address: @user.email_address, password: "password123" }
  end

  # Regression: the description was rendered with `t(".connect_desc_html", scope: ...)`.
  # `scope` is a reserved I18n option (it sets the lookup namespace), so the key never
  # resolved and Rails rendered the humanized fallback "Connect Desc Html".
  test "show renders the connect description, not a humanized placeholder, when configured" do
    with_env("GOOGLE_DRIVE_CLIENT_ID" => "test-client-id", "GOOGLE_DRIVE_CLIENT_SECRET" => "test-secret") do
      get settings_integrations_google_drive_path
    end

    assert_response :success
    assert_includes @response.body, "browse and pick any folder"
    assert_not_includes @response.body, "Connect Desc Html"
  end

  test "show explains Drive is unavailable, hiding the Connect button, when unconfigured" do
    with_env("GOOGLE_DRIVE_CLIENT_ID" => nil, "GOOGLE_DRIVE_CLIENT_SECRET" => nil) do
      get settings_integrations_google_drive_path
    end

    assert_response :success
    assert_includes @response.body, "Ask your administrator to configure it"
    assert_not_includes @response.body, "Connect Google Drive"
  end

  test "show links back to the integrations index" do
    with_env("GOOGLE_DRIVE_CLIENT_ID" => nil, "GOOGLE_DRIVE_CLIENT_SECRET" => nil) do
      get settings_integrations_google_drive_path
    end

    assert_select "a[href=?]", settings_integrations_root_path, text: /Back to integrations/
  end

  private

  # Set (or delete, when the value is nil) ENV keys for the block, then restore.
  def with_env(vars)
    original = ENV.to_hash
    vars.each { |k, v| ENV[k] = v }
    yield
  ensure
    ENV.replace(original)
  end
end

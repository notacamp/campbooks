require "test_helper"

class Oauth::GoogleControllerTest < ActionDispatch::IntegrationTest
  setup do
    @ws = Workspace.create!(name: "GDrive Connect WS", slug: "gd-conn-#{SecureRandom.hex(4)}")
    @user = @ws.users.create!(
      name: "GDrive Tester",
      email_address: "gd-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
    post session_path, params: { email_address: @user.email_address, password: "password123" }
  end

  # Regression: #connect built a GoogleDrive::OauthClient unconditionally, whose
  # initializer ENV.fetches the client id/secret and raised KeyError (→ 500) on an
  # instance that never configured Drive OAuth.
  test "connect redirects with a message instead of erroring when Drive OAuth is unconfigured" do
    with_env("GOOGLE_DRIVE_CLIENT_ID" => nil, "GOOGLE_DRIVE_CLIENT_SECRET" => nil) do
      get oauth_google_connect_path
    end

    assert_redirected_to settings_integrations_google_drive_path
    assert_includes flash[:warning], "Ask your administrator to configure it"
  end

  test "connect sends the user to Google's consent screen when configured" do
    with_env("GOOGLE_DRIVE_CLIENT_ID" => "test-client-id", "GOOGLE_DRIVE_CLIENT_SECRET" => "test-secret") do
      get oauth_google_connect_path
    end

    assert_response :redirect
    assert_match %r{//accounts\.google\.com/}, @response.headers["Location"]
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

require "test_helper"

class RegistrationsControllerTest < ActionDispatch::IntegrationTest
  # turbo-rails identifies the native shell by "(Turbo|Hotwire) Native" in the UA.
  NATIVE_UA = "Campbooks/1.0 iOS Hotwire Native".freeze
  WEB_SIGNUP_LINK = "https://campbooks.not-a-camp.com/".freeze

  setup do
    # The signup affordance only renders when public signup is allowed; pin the
    # mode so the test is deterministic regardless of SIGNUP_MODE in the env.
    @previous_signup_mode = Rails.application.config.signup_mode
    Rails.application.config.signup_mode = :open
  end

  teardown do
    Rails.application.config.signup_mode = @previous_signup_mode
  end

  test "web sign-in page links to in-app account creation" do
    get new_session_path

    assert_response :success
    assert_select "a[href=?]", new_registration_path
  end

  test "native sign-in page hides in-app signup and points to the web instead" do
    get new_session_path, headers: { "HTTP_USER_AGENT" => NATIVE_UA }

    assert_response :success
    assert_select "a[href=?]", new_registration_path, count: 0
    assert_select "a[href=?]", WEB_SIGNUP_LINK
  end

  test "native registration is blocked and redirects to sign-in" do
    get new_registration_path, headers: { "HTTP_USER_AGENT" => NATIVE_UA }

    assert_redirected_to new_session_path
    assert_equal I18n.t("registrations.native_signup_unavailable"), flash[:alert]
  end

  test "web registration renders the signup form" do
    get new_registration_path

    assert_response :success
  end
end

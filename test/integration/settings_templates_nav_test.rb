require "test_helper"

# The document- and email-template settings pages are readiness-gated features
# (Features.document_templates? / Features.email_templates?): their controllers
# `head :not_found` when the flag is off. The settings sidebar therefore must not
# advertise a link to them while gated off, or it dead-ends on a blank 404 page.
class SettingsTemplatesNavTest < ActionDispatch::IntegrationTest
  setup do
    @ws = Workspace.create!(name: "Nav WS", slug: "nav-#{SecureRandom.hex(4)}")
    @user = @ws.users.create!(
      name: "Nav Tester",
      email_address: "nav-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
    post session_path, params: { email_address: @user.email_address, password: "password123" }
  end

  test "hides the template links when the readiness flags are off (the default)" do
    get settings_root_path
    assert_response :success
    # Sanity: the AI & Automation group renders, so the absences below are real.
    assert_includes @response.body, settings_ai_path
    assert_not_includes @response.body, settings_document_templates_path
    assert_not_includes @response.body, settings_email_templates_path
  end

  test "shows the template links when the readiness flags are on" do
    # Features reads ENV at call time; flip the flags for this request and restore.
    keys = %w[ENABLE_DOCUMENT_TEMPLATES ENABLE_EMAIL_TEMPLATES]
    saved = keys.index_with { |k| ENV[k] }
    begin
      keys.each { |k| ENV[k] = "1" }
      get settings_root_path
      assert_response :success
      assert_includes @response.body, settings_document_templates_path
      assert_includes @response.body, settings_email_templates_path
    ensure
      saved.each { |k, v| v.nil? ? ENV.delete(k) : (ENV[k] = v) }
    end
  end
end

require "rails_helper"

# The document- and email-template settings pages are readiness-gated features
# (Features.document_templates? / Features.email_templates?): their controllers
# `head :not_found` when the flag is off. The settings sidebar therefore must not
# advertise a link to them while gated off, or it dead-ends on a blank 404 page.
RSpec.describe "SettingsTemplatesNav", type: :request do
  let(:ws) { Workspace.create!(name: "Nav WS", slug: "nav-#{SecureRandom.hex(4)}") }
  let(:user) do
    ws.users.create!(
      name: "Nav Tester",
      email_address: "nav-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
  end

  before { sign_in(user) }

  it "hides the template links when the readiness flags are off (the default)" do
    get settings_root_path
    expect(response).to have_http_status(:ok)
    # Sanity: the AI & Automation group renders, so the absences below are real.
    expect(response.body).to include(settings_ai_path)
    expect(response.body).not_to include(settings_document_templates_path)
    expect(response.body).not_to include(settings_email_templates_path)
  end

  it "shows the template links when the readiness flags are on" do
    # Features reads ENV at call time; flip the flags for this request and restore.
    keys = %w[ENABLE_DOCUMENT_TEMPLATES ENABLE_EMAIL_TEMPLATES]
    with_env(keys.index_with { "1" }) do
      get settings_root_path
      expect(response).to have_http_status(:ok)
      expect(response.body).to include(settings_document_templates_path)
      expect(response.body).to include(settings_email_templates_path)
    end
  end
end

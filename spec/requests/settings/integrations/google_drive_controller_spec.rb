require "rails_helper"

RSpec.describe "Settings::Integrations::GoogleDriveController", type: :request do
  before do
    @ws = Workspace.create!(name: "GDrive Show WS", slug: "gd-show-#{SecureRandom.hex(4)}")
    @user = @ws.users.create!(
      name: "GDrive Show Tester",
      email_address: "gds-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
    sign_in(@user)
  end

  # Regression: the description was rendered with `t(".connect_desc_html", scope: ...)`.
  # `scope` is a reserved I18n option (it sets the lookup namespace), so the key never
  # resolved and Rails rendered the humanized fallback "Connect Desc Html".
  it "show renders the connect description, not a humanized placeholder, when configured" do
    with_env("GOOGLE_DRIVE_CLIENT_ID" => "test-client-id", "GOOGLE_DRIVE_CLIENT_SECRET" => "test-secret") do
      get settings_integrations_google_drive_path
    end

    expect(response).to have_http_status(:ok)
    expect(response.body).to include("browse and pick any folder")
    expect(response.body).not_to include("Connect Desc Html")
  end

  it "show explains Drive is unavailable, hiding the Connect button, when unconfigured" do
    with_env("GOOGLE_DRIVE_CLIENT_ID" => nil, "GOOGLE_DRIVE_CLIENT_SECRET" => nil) do
      get settings_integrations_google_drive_path
    end

    expect(response).to have_http_status(:ok)
    expect(response.body).to include("Ask your administrator to configure it")
    expect(response.body).not_to include("Connect Google Drive")
  end

  it "show links back to the integrations index" do
    with_env("GOOGLE_DRIVE_CLIENT_ID" => nil, "GOOGLE_DRIVE_CLIENT_SECRET" => nil) do
      get settings_integrations_google_drive_path
    end

    expect(response.body).to include(settings_integrations_root_path)
    expect(response.body).to match(/Back to integrations/)
  end
end

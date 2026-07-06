require "rails_helper"

RSpec.describe "Oauth::GoogleController", type: :request do
  before do
    @ws = Workspace.create!(name: "GDrive Connect WS", slug: "gd-conn-#{SecureRandom.hex(4)}")
    @user = @ws.users.create!(
      name: "GDrive Tester",
      email_address: "gd-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
    sign_in(@user)
  end

  # Regression: #connect built a GoogleDrive::OauthClient unconditionally, whose
  # initializer ENV.fetches the client id/secret and raised KeyError (→ 500) on an
  # instance that never configured Drive OAuth.
  it "connect redirects with a message instead of erroring when Drive OAuth is unconfigured" do
    with_env("GOOGLE_DRIVE_CLIENT_ID" => nil, "GOOGLE_DRIVE_CLIENT_SECRET" => nil) do
      get oauth_google_connect_path
    end

    expect(response).to redirect_to(settings_integrations_google_drive_path)
    expect(flash[:warning]).to include("Ask your administrator to configure it")
  end

  it "connect sends the user to Google's consent screen when configured" do
    with_env("GOOGLE_DRIVE_CLIENT_ID" => "test-client-id", "GOOGLE_DRIVE_CLIENT_SECRET" => "test-secret") do
      get oauth_google_connect_path
    end

    expect(response).to have_http_status(:redirect)
    expect(response.headers["Location"]).to match(%r{//accounts\.google\.com/})
  end
end

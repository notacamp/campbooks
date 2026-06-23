require "rails_helper"

RSpec.describe "Settings data export", type: :request do
  it "streams the signed-in user's personal data as a JSON download" do
    user = create(:user)
    sign_in(user)

    get export_settings_account_path

    expect(response).to have_http_status(:ok)
    expect(response.media_type).to eq("application/json")
    expect(response.headers["Content-Disposition"]).to include("attachment")
    body = JSON.parse(response.body)
    expect(body.dig("account", "email_address")).to eq(user.email_address)
    expect(body).to have_key("ai_conversations")
  end

  it "requires authentication" do
    get export_settings_account_path
    expect(response).to redirect_to("/session/new")
  end
end

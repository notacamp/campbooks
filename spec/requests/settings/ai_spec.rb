require "rails_helper"

RSpec.describe "Settings AI providers", type: :request do
  it "renders the AI settings page with Mistral (EU) and per-provider data regions" do
    sign_in(create(:user))

    get settings_ai_path

    expect(response).to have_http_status(:ok)
    expect(response.body).to include("Mistral")             # new provider is selectable
    expect(response.body).to match(/Mistral.{0,8}EU/m)      # region surfaced at the point of choice
    expect(response.body).to match(/DeepSeek.{0,12}China/m) # non-EU transfer disclosed
  end
end

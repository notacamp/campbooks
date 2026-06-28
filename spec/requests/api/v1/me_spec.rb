require "rails_helper"

RSpec.describe "API v1 me", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace, name: "Ada Byron", email_address: "ada@example.com") }

  it "returns the acting user, workspace, and granted scopes" do
    headers = api_auth_headers(workspace: workspace, user: user, scopes: "emails:read documents:read")

    get api_v1_me_path, headers: headers

    expect(response).to have_http_status(:ok)
    data = response.parsed_body["data"]
    expect(data.dig("user", "id")).to eq(user.id)
    expect(data.dig("user", "name")).to eq("Ada Byron")
    expect(data.dig("user", "email")).to eq("ada@example.com")
    expect(data.dig("workspace", "id")).to eq(workspace.id)
    expect(data.dig("workspace", "name")).to eq(workspace.name)
    expect(data["scopes"]).to contain_exactly("emails:read", "documents:read")
  end

  it "401s without a token" do
    get api_v1_me_path

    expect(response).to have_http_status(:unauthorized)
    expect(response.parsed_body.dig("error", "code")).to eq("invalid_token")
  end
end

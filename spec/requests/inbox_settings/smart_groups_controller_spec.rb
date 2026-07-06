require "rails_helper"

RSpec.describe "InboxSettings::SmartGroupsController", type: :request do
  let(:workspace) { Workspace.create!(name: "SG Settings WS") }
  let(:user) do
    workspace.users.create!(
      name: "Ana",
      email_address: "ana-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
  end

  before { sign_in(user) }

  it "requires authentication" do
    delete "/session"
    get inbox_settings_smart_groups_path
    expect(response).to redirect_to(new_session_path)
  end

  it "show renders the panel" do
    get inbox_settings_smart_groups_path
    expect(response).to have_http_status(:ok)
    expect(response.body).to match("smart_groups_panel")
  end

  it "update persists prefs and ignores unknown keys" do
    patch inbox_settings_smart_groups_path, params: {
      smart_groups: { enabled: "1", promotions: "0", social: "1", bogus: "1" }
    }

    prefs = user.reload.inbox_smart_groups
    expect(prefs["enabled"]).to eq(true)
    expect(prefs["promotions"]).to eq(false)
    expect(prefs["social"]).to eq(true)
    expect(prefs["bogus"]).to be_nil
    expect(user.smart_group_enabled?("promotions")).to be_falsey
    expect(user.smart_group_enabled?("notifications")).to be_truthy
  end

  it "update can disable the whole feature" do
    patch inbox_settings_smart_groups_path, params: { smart_groups: { enabled: "0" } }
    expect(user.reload.smart_groups_enabled?).to be_falsey
    expect(user.enabled_smart_group_buckets).to eq([])
  end
end

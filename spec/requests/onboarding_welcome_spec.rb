require "rails_helper"

# The post-signup golden path: /onboarding opens on the Scout welcome screen
# (connect an inbox, one optional toggle), the legacy wizard steps stay
# reachable, and the first-sync status endpoint feeds the live stage on home.
RSpec.describe "Onboarding welcome flow", type: :request do
  let(:user) { create(:user) }

  before { sign_in(user) }

  describe "GET /onboarding" do
    it "renders the welcome step by default" do
      get onboarding_path
      expect(response).to have_http_status(:ok)
      expect(response.body).to include("Scout")
      expect(response.body).to include(email_accounts_path(provider: :google))
    end

    it "still serves the legacy wizard steps" do
      get onboarding_path(step: :workspace)
      expect(response).to have_http_status(:ok)

      get onboarding_path(step: :email_accounts)
      expect(response).to have_http_status(:ok)
    end
  end

  describe "PATCH /onboarding (welcome toggle)" do
    it "saves the workspace type in place and returns to the welcome screen" do
      patch onboarding_path, params: { step: "welcome", workspace: { workspace_type: "individual" } }
      expect(response).to redirect_to(onboarding_path)
      expect(user.workspace.reload.workspace_type).to eq("individual")
      expect(user.workspace.settings["workspace_type"]).to eq("individual")
    end

    it "ignores unknown workspace types" do
      patch onboarding_path, params: { step: "welcome", workspace: { workspace_type: "pineapple" } }
      expect(response).to redirect_to(onboarding_path)
      expect(user.workspace.reload.settings["workspace_type"]).not_to eq("pineapple")
    end
  end

  describe "GET /onboarding/first_sync_status" do
    it "reports waiting before any scan exists" do
      account = create(:email_account, workspace: user.workspace)
      account.email_account_users.create!(user: user, owner: true, can_read: true, can_send: true, can_manage: true)

      get first_sync_status_onboarding_path
      expect(response).to have_http_status(:ok)
      expect(response.parsed_body).to include("state" => "waiting", "found" => 0)
    end

    it "reports scanning while the first scan runs, then done with counts" do
      account = create(:email_account, workspace: user.workspace)
      account.email_account_users.create!(user: user, owner: true, can_read: true, can_send: true, can_manage: true)
      log = EmailScanLog.create!(email_account: account, status: :running, started_at: Time.current)

      get first_sync_status_onboarding_path
      expect(response.parsed_body["state"]).to eq("scanning")

      create(:email_message, email_account: account, category: "important", ai_priority: :high)
      log.update!(status: :completed, completed_at: Time.current)

      get first_sync_status_onboarding_path
      expect(response.parsed_body).to include("state" => "done", "found" => 1, "sorted" => 1, "needs_you" => 1)
    end
  end

  describe "home first-sync stage" do
    it "takes over home while the first scan runs and steps aside after it completes" do
      account = create(:email_account, workspace: user.workspace)
      account.email_account_users.create!(user: user, owner: true, can_read: true, can_send: true, can_manage: true)
      log = EmailScanLog.create!(email_account: account, status: :running, started_at: Time.current)

      get root_path
      expect(response.body).to include("data-controller=\"first-sync\"")

      log.update!(status: :completed, completed_at: Time.current)
      get root_path
      expect(response.body).not_to include("data-controller=\"first-sync\"")
    end
  end
end

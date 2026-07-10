require "rails_helper"

# The post-signup golden path: /onboarding now opens on the template picker
# (step 0 — "What will you mostly use Campbooks for?"). The Scout welcome screen
# (connect an inbox) is still reachable as step=welcome. Legacy wizard steps stay
# reachable, and the first-sync status endpoint feeds the live stage on home.
RSpec.describe "Onboarding welcome flow", type: :request do
  let(:user) { create(:user) }

  before { sign_in(user) }

  describe "GET /onboarding" do
    it "renders the template picker by default (step 0)" do
      get onboarding_path
      expect(response).to have_http_status(:ok)
      # Distinctive heading from the template step
      expect(response.body).to include("What will you mostly use Campbooks for")
      # All five persona cards are present
      expect(response.body).to include("Freelancer")
      expect(response.body).to include("Just exploring")
      # Prominent skip is present
      expect(response.body).to include("Skip for now")
    end

    it "renders the welcome step when step=welcome" do
      get onboarding_path(step: :welcome)
      expect(response).to have_http_status(:ok)
      # The welcome screen has the Scout avatar and inbox connect buttons
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

    it "renders the escape-hatch form POSTing to skip_first_sync_onboarding_path" do
      account = create(:email_account, workspace: user.workspace)
      account.email_account_users.create!(user: user, owner: true, can_read: true, can_send: true, can_manage: true)
      EmailScanLog.create!(email_account: account, status: :running, started_at: Time.current)

      get root_path
      expect(response.body).to include(skip_first_sync_onboarding_path)
      # The form carries the Stimulus escape target
      expect(response.body).to include("first-sync-target=\"escape\"")
    end

    it "renders the persona card with the apply_persona endpoint" do
      account = create(:email_account, workspace: user.workspace)
      account.email_account_users.create!(user: user, owner: true, can_read: true, can_send: true, can_manage: true)
      EmailScanLog.create!(email_account: account, status: :running, started_at: Time.current)

      get root_path
      expect(response.body).to include("first-sync-persona")
      expect(response.body).to include(apply_persona_onboarding_path)
    end
  end
end

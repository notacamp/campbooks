# frozen_string_literal: true

require "rails_helper"

RSpec.describe "POST /onboarding/skip_first_sync", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }

  # An active account with no completed scan — this is what makes @first_sync.stage? true.
  def setup_mid_scan_account
    account = create(:email_account, workspace: workspace)
    account.email_account_users.create!(
      user: user, owner: true,
      can_read: true, can_send: true, can_manage: true
    )
    EmailScanLog.create!(email_account: account, status: :running, started_at: Time.current)
    account
  end

  before { sign_in(user) }

  describe "POST /onboarding/skip_first_sync" do
    it "redirects to the inbox" do
      post skip_first_sync_onboarding_path
      expect(response).to redirect_to(email_messages_path)
    end

    it "sets the first_sync_skipped session flag" do
      post skip_first_sync_onboarding_path
      # Follow the redirect so the session cookie is re-read
      follow_redirect!
      # The session flag is now set; verify home doesn't re-render the stage
      setup_mid_scan_account
      get root_path
      expect(response.body).not_to include("data-controller=\"first-sync\"")
    end
  end

  describe "home behaviour after skip" do
    it "does NOT render the first-sync stage even while stage? is true" do
      setup_mid_scan_account
      # First visit without skip — stage is shown
      get root_path
      expect(response.body).to include("data-controller=\"first-sync\"")

      # Now skip
      post skip_first_sync_onboarding_path
      follow_redirect!

      # Back to home — should render normal home, not the stage
      get root_path
      expect(response.body).not_to include("data-controller=\"first-sync\"")
    end

    it "shows the :syncing inbox state copy when the scan is still running" do
      setup_mid_scan_account
      post skip_first_sync_onboarding_path
      follow_redirect!

      get root_path
      # The syncing empty-state copy, NOT "All caught up"
      expect(response.body).to include("Scout is reading your inbox")
      expect(response.body).not_to include("All caught up")
    end
  end

  describe "authentication" do
    it "requires a signed-in user" do
      delete "/session"
      post skip_first_sync_onboarding_path
      expect(response).to redirect_to(new_session_path)
    end
  end
end

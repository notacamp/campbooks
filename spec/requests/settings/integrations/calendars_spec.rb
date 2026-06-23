require "rails_helper"

RSpec.describe "Settings::Integrations::Calendars", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  describe "GET /settings/integrations/calendars" do
    it "redirects unauthenticated users to sign-in" do
      get settings_integrations_calendars_path
      expect(response).to redirect_to(new_session_path)
    end

    it "renders the empty state when no calendars are connected" do
      sign_in(user)
      get settings_integrations_calendars_path
      expect(response).to have_http_status(:ok)
    end

    # Regression: the page 500'd in prod (undefined method `calendar_accounts_path`)
    # whenever at least one calendar account was connected — the "Reconnect" button
    # referenced a route helper that doesn't exist (calendar_accounts has no :create).
    it "renders the connected-accounts list, including the Reconnect button" do
      account = create(:calendar_account, workspace: workspace)
      create(:calendar_account_user, :owner, user: user, calendar_account: account)
      create(:calendar, calendar_account: account)

      sign_in(user)
      get settings_integrations_calendars_path

      expect(response).to have_http_status(:ok)
      # Reconnect rides the email-account OAuth grant (no separate calendar connect).
      expect(response.body).to include(email_accounts_path(provider: account.provider))
    end
  end
end

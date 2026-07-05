require "rails_helper"

# /jobs (Mission Control) must not be publicly reachable: the queue dashboard
# surfaces job arguments that can contain personal data (across every
# workspace), and lets callers retry or discard jobs. It is gated to instance
# operators (app_admin) via MissionControlController — a workspace admin has
# no business here.
RSpec.describe "Mission Control Jobs dashboard", type: :request do
  describe "GET /jobs" do
    it "bounces anonymous visitors to sign in" do
      get "/jobs"
      expect(response).to redirect_to("/session/new")
    end

    it "forbids authenticated non-admin users" do
      sign_in(create(:user))
      get "/jobs"
      expect(response).to have_http_status(:forbidden)
    end

    it "forbids workspace admins without the app_admin flag" do
      sign_in(create(:user, role: :admin))
      get "/jobs"
      expect(response).to have_http_status(:forbidden)
    end

    it "admits app admins" do
      sign_in(create(:user, app_admin: true))
      get "/jobs"
      expect(response).not_to have_http_status(:forbidden)
      expect(response).not_to redirect_to("/session/new")
    end
  end
end

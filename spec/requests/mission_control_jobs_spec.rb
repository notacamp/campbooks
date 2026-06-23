require "rails_helper"

# /jobs (Mission Control) must not be publicly reachable: the queue dashboard
# surfaces job arguments that can contain personal data, and lets callers retry
# or discard jobs. It is gated to admins via MissionControlController.
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

    it "admits admins" do
      sign_in(create(:user, role: :admin))
      get "/jobs"
      expect(response).not_to have_http_status(:forbidden)
      expect(response).not_to redirect_to("/session/new")
    end
  end
end

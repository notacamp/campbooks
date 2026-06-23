require "rails_helper"

# A signed session cookie is permanent, so the server-side inactivity window is
# what actually expires a login. After Session::INACTIVITY_LIMIT idle, the next
# request must re-authenticate and the stale row (ip_address/user_agent) is gone.
RSpec.describe "Session expiry", type: :request do
  it "logs out and prunes a session idle past the inactivity limit" do
    user = create(:user)
    sign_in(user)

    get settings_account_path
    expect(response).not_to redirect_to("/session/new")

    user.sessions.update_all(updated_at: (Session::INACTIVITY_LIMIT + 1.day).ago)

    get settings_account_path
    expect(response).to redirect_to("/session/new")
    expect(Session.where(user: user)).to be_empty
  end
end

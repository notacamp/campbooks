require "rails_helper"

RSpec.describe "Native session exchange (SessionsController#native)", type: :request do
  let(:user) { create(:user) }

  it "redeems a valid token: starts a session and redirects home" do
    token = user.generate_token_for(:native_session)

    expect {
      get native_session_path(token: token)
    }.to change { user.sessions.count }.by(1)

    expect(response).to redirect_to(root_url)
  end

  it "rejects an invalid token (and an unauthenticated request still reaches the action)" do
    # No sign_in here: the endpoint is public, so an invalid token lands on the
    # login page rather than being blocked by the auth gate.
    expect {
      get native_session_path(token: "garbage")
    }.not_to change { user.sessions.count }

    expect(response).to redirect_to("/session/new")
  end
end

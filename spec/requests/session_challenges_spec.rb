require "rails_helper"

RSpec.describe "SessionChallenges", type: :request do
  include ActiveSupport::Testing::TimeHelpers

  let(:secret) { ROTP::Base32.random }
  let(:user)   { create(:user) }

  before { user.update!(totp_secret: secret, totp_enabled_at: Time.current) }

  def login
    post session_path, params: { email_address: user.email_address, password: "password123" }
  end

  it "parks an MFA user at the challenge instead of signing them in" do
    login
    expect(response).to redirect_to(session_challenge_path)
  end

  it "renders the TOTP challenge once a login is pending" do
    login
    get session_challenge_path

    expect(response).to have_http_status(:ok)
    expect(response.body).to include(I18n.t("session_challenges.show.title"))
  end

  it "redirects to sign-in when there is no pending challenge" do
    get session_challenge_path
    expect(response).to redirect_to(new_session_path)
  end

  it "completes sign-in with a valid TOTP code" do
    login
    post session_challenge_path, params: { method: "totp", code: ROTP::TOTP.new(secret).now }

    expect(response).to redirect_to(root_url)
  end

  it "rejects a wrong TOTP code" do
    login
    post session_challenge_path, params: { method: "totp", code: "000000" }

    expect(response).to have_http_status(:unprocessable_entity)
  end

  it "rejects a replayed TOTP code on a later login" do
    code = ROTP::TOTP.new(secret).now

    login
    post session_challenge_path, params: { method: "totp", code: code }
    expect(response).to redirect_to(root_url)

    delete session_path # sign out so the next login isn't a no-op

    login
    post session_challenge_path, params: { method: "totp", code: code }
    expect(response).to have_http_status(:unprocessable_entity)
  end

  it "accepts a recovery code and consumes it" do
    codes = RecoveryCode.regenerate_for!(user)

    login
    expect {
      post session_challenge_path, params: { method: "recovery", code: codes.first }
    }.to change { user.recovery_codes.unused.count }.by(-1)

    expect(response).to redirect_to(root_url)
  end

  it "expires a stale pending challenge" do
    login
    travel(11.minutes) do
      post session_challenge_path, params: { method: "totp", code: ROTP::TOTP.new(secret).now }
      expect(response).to redirect_to(new_session_path)
    end
  end
end

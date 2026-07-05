require "rails_helper"

# The 2FA challenge gates the password path (SessionsController#create) AND web
# OAuth sign-in (Google/Microsoft/Zoho): a user who enabled app MFA must clear it
# even when signing in through the browser. Native-token sign-in is the documented
# exception — it still relies on the provider's own MFA (an in-webview challenge is
# a separate effort, and the handoff already needs the installed app + a one-time
# token).
RSpec.describe "MFA on non-password sign-in", type: :request do
  # The Zoho OAuth callback constructs a client (which ENV.fetches its app
  # credentials) before the MFA gate runs; supply dummies so it reaches the gate
  # instead of raising KeyError (CI has no real keys).
  around { |example| with_env("ZOHO_CLIENT_ID" => "test-id", "ZOHO_CLIENT_SECRET" => "test-secret") { example.run } }

  let(:user) { create(:user) }

  before { user.update!(totp_secret: ROTP::Base32.random, totp_enabled_at: Time.current) }

  it "challenges the password path when 2FA is on" do
    post session_path, params: { email_address: user.email_address, password: "password123" }
    expect(response).to redirect_to(session_challenge_path)
  end

  it "challenges web OAuth sign-in when 2FA is on" do
    # OAuth signs a user in only through a LINKED identity (Auth::OauthSignIn);
    # an unlinked email match blocks instead. Link it so this exercises the real
    # sign-in path, where the MFA gate must still fire.
    create(:identity, user: user, provider: "zoho", uid: "acct-1")
    allow_any_instance_of(Zoho::OauthClient).to receive(:exchange_code).and_return("access_token" => "tok")
    allow_any_instance_of(Zoho::AccountDiscovery).to receive(:discover_identity)
      .and_return(email: user.email_address, name: user.name, account_id: "acct-1")

    get "/oauth/zoho/callback", params: { code: "abc", state: Oauth::State.encode(flow: "sign_in") }

    expect(response).to redirect_to(session_challenge_path)
  end

  it "does NOT challenge native-token sign-in even with 2FA on (documented exception)" do
    token = user.generate_token_for(:native_session)

    get native_session_path(token: token)

    expect(response).to redirect_to(root_url)
    expect(response).not_to redirect_to(session_challenge_path)
  end
end

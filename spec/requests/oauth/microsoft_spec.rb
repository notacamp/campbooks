require "rails_helper"

RSpec.describe "Microsoft OAuth", type: :request do
  # Microsoft::OauthClient reads these in #initialize / .authorize_url; they aren't
  # set in the test env, so stub them rather than depend on a real Entra app.
  before do
    allow(ENV).to receive(:fetch).and_call_original
    allow(ENV).to receive(:fetch).with("MICROSOFT_CLIENT_ID").and_return("test-ms-client-id")
    allow(ENV).to receive(:fetch).with("MICROSOFT_CLIENT_SECRET").and_return("test-ms-secret")
  end

  def state_param(location)
    Oauth::State.decode(CGI.parse(URI(location).query)["state"].first)
  end

  describe "GET /session/microsoft (Sign in with Microsoft)" do
    it "redirects to Microsoft's authorize URL with a signed sign_in state" do
      get microsoft_session_path

      expect(response).to have_http_status(:redirect)
      expect(response.location).to start_with("https://login.microsoftonline.com/")
      expect(response.location).to include("client_id=test-ms-client-id")
      expect(state_param(response.location)).to include("flow" => "sign_in", "native" => false, "verified" => true)
    end

    it "marks the state native: true when the request comes from the native app" do
      get microsoft_session_path, headers: { "HTTP_USER_AGENT" => "Campbooks Hotwire Native iOS" }
      expect(state_param(response.location)).to include("flow" => "sign_in", "native" => true)
    end
  end

  describe "POST /email_accounts?provider=microsoft (connect a mailbox)" do
    let(:user) { create(:user) }

    # Microsoft mailbox connect is hidden/disabled until the Entra app is wired up
    # (ENABLE_MICROSOFT_MAILBOX=1). ENV["..."] uses #[], not #fetch.
    context "when Microsoft mailbox connect is enabled" do
      before do
        allow(ENV).to receive(:[]).and_call_original
        allow(ENV).to receive(:[]).with("ENABLE_MICROSOFT_MAILBOX").and_return("1")
      end

      it "redirects an authenticated user to Microsoft with an account_link state" do
        sign_in(user)
        post email_accounts_path(provider: "microsoft")

        expect(response).to have_http_status(:redirect)
        expect(response.location).to start_with("https://login.microsoftonline.com/")
        expect(state_param(response.location)).to include("flow" => "account_link", "native" => false)
      end

      it "embeds the signed user id in the state for a native connect" do
        sign_in(user)
        post email_accounts_path(provider: "microsoft"),
             headers: { "HTTP_USER_AGENT" => "Campbooks Hotwire Native Android" }

        expect(state_param(response.location)).to include(
          "flow" => "account_link", "native" => true, "user_id" => user.id, "verified" => true
        )
      end
    end

    context "when Microsoft mailbox connect is disabled (default)" do
      it "refuses the connect and does not redirect to Microsoft" do
        sign_in(user)
        post email_accounts_path(provider: "microsoft")

        expect(response).to have_http_status(:redirect)
        expect(response.location).not_to include("login.microsoftonline.com")
      end
    end
  end

  describe "GET /oauth/microsoft/callback" do
    let(:token_data) { { "access_token" => "ms-access", "refresh_token" => "ms-refresh" } }

    before do
      allow_any_instance_of(Microsoft::OauthClient).to receive(:exchange_code).and_return(token_data)
      allow_any_instance_of(Microsoft::AccountDiscovery).to receive(:discover_identity).and_return(identity)
    end

    context "sign-in flow (runs unauthenticated)" do
      let(:identity) { { email: "alice@contoso.com", name: "Alice", account_id: "ms-oid-1" } }

      it "provisions the user in a fresh personal workspace, links the identity, and starts a session" do
        expect {
          get oauth_microsoft_callback_path, params: { code: "auth-code", state: { flow: "sign_in" }.to_json }
        }.to change(User, :count).by(1)

        user = User.find_by(email_address: "alice@contoso.com")
        expect(user).to be_present
        expect(user.name).to eq("Alice")
        # Fresh per-user workspace — NOT grouped by the "contoso.com" domain.
        expect(user.workspace.slug).to start_with("ws-")
        expect(user.identities.pluck(:provider, :uid)).to eq([ [ "microsoft", "ms-oid-1" ] ])
        expect(user.sessions.count).to eq(1)
        expect(response).to have_http_status(:redirect)
      end
    end

    context "sign-in collisions (block + guide — no session, no duplicate user)" do
      # Default discovered identity: a NEW uid at an email that will already be
      # taken (by a user or a mailbox) in each example below.
      let(:identity) { { email: "alice@contoso.com", name: "Alice", account_id: "ms-new" } }

      it "signs into the linked user when an Identity matches — no new user" do
        existing = create(:user)
        create(:identity, user: existing, provider: "microsoft", uid: "ms-oid-1")
        allow_any_instance_of(Microsoft::AccountDiscovery).to receive(:discover_identity)
          .and_return(email: "anything@contoso.com", name: "Alice", account_id: "ms-oid-1")

        expect {
          get oauth_microsoft_callback_path, params: { code: "auth-code", state: { flow: "sign_in" }.to_json }
        }.not_to change(User, :count)
        expect(existing.sessions.count).to eq(1)
      end

      it "blocks when the email already belongs to an account (no linked identity)" do
        create(:user, email_address: "alice@contoso.com")

        expect {
          get oauth_microsoft_callback_path, params: { code: "auth-code", state: { flow: "sign_in" }.to_json }
        }.not_to change(User, :count)
        expect(response).to redirect_to("/session/new")
      end

      it "blocks when the email is a connected mailbox (no user account)" do
        create(:email_account, email_address: "alice@contoso.com", provider: :microsoft)

        expect {
          get oauth_microsoft_callback_path, params: { code: "auth-code", state: { flow: "sign_in" }.to_json }
        }.not_to change(User, :count)
        expect(response).to redirect_to("/session/new")
      end
    end

    context "account-link flow (requires an authenticated user)" do
      let(:user) { create(:user) }
      let(:identity) { { email: "shared@contoso.com", name: "Shared Mailbox", account_id: "ms-oid-2" } }

      it "creates a Microsoft EmailAccount owned by the current user" do
        sign_in(user)

        expect {
          get oauth_microsoft_callback_path, params: { code: "auth-code", state: { flow: "account_link" }.to_json }
        }.to change(EmailAccount, :count).by(1)

        account = EmailAccount.find_by(email_address: "shared@contoso.com")
        expect(account.provider).to eq("microsoft")
        expect(account.provider_account_id).to eq("ms-oid-2")
        expect(account.active).to be(true)
        expect(account.owned_by?(user)).to be(true)
      end

      it "blocks the callback when no user is signed in" do
        expect {
          get oauth_microsoft_callback_path, params: { code: "auth-code", state: { flow: "account_link" }.to_json }
        }.not_to change(EmailAccount, :count)

        expect(response).to redirect_to("/session/new")
      end
    end

    context "native sign-in flow (system auth session — no session cookie)" do
      let(:identity) { { email: "alice@contoso.com", name: "Alice", account_id: "ms-oid-1" } }

      it "provisions the user, mints a one-time token, and hands back via campbooks://" do
        state = Oauth::State.encode(flow: "sign_in", native: true)

        expect {
          get oauth_microsoft_callback_path, params: { code: "auth-code", state: state }
        }.to change(User, :count).by(1)

        user = User.find_by(email_address: "alice@contoso.com")
        expect(user.sessions.count).to eq(0) # no web session created inside the auth session
        expect(response.location).to start_with("campbooks://oauth?")
        expect(response.location).to include("flow=signin")
        expect(response.location).to include("token=")
      end
    end

    context "native account-link flow (identity from the signed state — no cookie)" do
      let(:user) { create(:user) }
      let(:identity) { { email: "shared@contoso.com", name: "Shared Mailbox", account_id: "ms-oid-2" } }

      it "links the account to the signed-state user and hands back via campbooks://" do
        state = Oauth::State.encode(flow: "account_link", native: true, user_id: user.id)

        expect {
          get oauth_microsoft_callback_path, params: { code: "auth-code", state: state }
        }.to change(EmailAccount, :count).by(1)

        account = EmailAccount.find_by(email_address: "shared@contoso.com")
        expect(account.owned_by?(user)).to be(true)
        expect(response.location).to start_with("campbooks://oauth?")
        expect(response.location).to include("flow=connect")
      end
    end

    context "add-sign-in flow (link a provider to the current account)" do
      let(:user) { create(:user) }
      let(:identity) { { email: "me@contoso.com", name: "Me", account_id: "ms-add-1" } }

      it "links the Microsoft identity to the signed-in user (web)" do
        sign_in(user)

        expect {
          get oauth_microsoft_callback_path, params: { code: "auth-code", state: { flow: "add_sign_in" }.to_json }
        }.to change { user.identities.where(provider: "microsoft", uid: "ms-add-1").count }.by(1)

        expect(response).to redirect_to(settings_security_path)
      end

      it "links via the signed state in the native shell (no cookie)" do
        state = Oauth::State.encode(flow: "add_sign_in", native: true, user_id: user.id)

        expect {
          get oauth_microsoft_callback_path, params: { code: "auth-code", state: state }
        }.to change { user.identities.where(uid: "ms-add-1").count }.by(1)

        expect(response.location).to start_with("campbooks://oauth?")
        expect(response.location).to include("flow=add_sign_in")
      end
    end
  end
end

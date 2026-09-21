# frozen_string_literal: true

require "rails_helper"

# rubocop:disable RSpec/MultipleMemoizedHelpers

RSpec.describe "Api::App email accounts — mailbox connect", type: :request do
  let(:workspace) { create(:workspace) }
  let(:owner)     { create(:user, workspace: workspace) }

  # ── Auth guard ────────────────────────────────────────────────────────────────

  describe "GET /api/app/email_accounts (auth guard)" do
    it "returns 401 with no token" do
      get "/api/app/email_accounts"
      expect(response).to have_http_status(:unauthorized)
      expect(response.parsed_body.dig("error", "code")).to eq("unauthenticated")
    end

    it "returns 401 with a tampered token" do
      get "/api/app/email_accounts",
          headers: { "Authorization" => "Bearer this.is.fake" }
      expect(response).to have_http_status(:unauthorized)
    end
  end

  # ── GET /api/app/email_accounts ──────────────────────────────────────────────

  describe "GET /api/app/email_accounts" do
    let!(:account)         { create(:email_account, workspace: workspace) }
    let!(:other_ws_account) { create(:email_account) } # different workspace

    before { create(:email_account_user, :owner, user: owner, email_account: account) }

    it "lists only the accounts the current user can read" do
      get "/api/app/email_accounts", headers: api_app_headers(owner)

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data.length).to eq(1)
      expect(data.first["id"]).to eq(account.id)
    end

    it "includes permission fields on each account" do
      get "/api/app/email_accounts", headers: api_app_headers(owner)

      account_json = response.parsed_body["data"].first
      expect(account_json).to include(
        "can_read"   => true,
        "can_send"   => true,
        "can_manage" => true,
        "is_owner"   => true
      )
    end

    it "does not return inactive accounts" do
      account.update!(active: false)
      get "/api/app/email_accounts", headers: api_app_headers(owner)
      expect(response.parsed_body["data"]).to be_empty
    end
  end

  # ── DELETE /api/app/email_accounts/:id ───────────────────────────────────────

  describe "DELETE /api/app/email_accounts/:id" do
    let!(:account) { create(:email_account, workspace: workspace) }
    let(:sharee)   { create(:user, workspace: workspace) }

    before do
      create(:email_account_user, :owner,        user: owner,  email_account: account)
      create(:email_account_user, :collaborator, user: sharee, email_account: account)
    end

    it "lets the owner disconnect the account" do
      expect do
        delete "/api/app/email_accounts/#{account.id}",
               headers: api_app_headers(owner)
      end.to have_enqueued_job(EmailAccountRemovalJob).with(account.id)

      expect(response).to have_http_status(:no_content)
      expect(account.reload.active).to be(false)
    end

    it "returns 404 for a non-owner (404-not-403 rule)" do
      expect do
        delete "/api/app/email_accounts/#{account.id}",
               headers: api_app_headers(sharee)
      end.not_to have_enqueued_job(EmailAccountRemovalJob)

      expect(response).to have_http_status(:not_found)
      expect(account.reload.active).to be(true)
    end

    it "returns 404 for an account in a different workspace" do
      other = create(:email_account)
      delete "/api/app/email_accounts/#{other.id}",
             headers: api_app_headers(owner)
      expect(response).to have_http_status(:not_found)
    end
  end

  # ── GET /api/app/oauth/authorize_url ─────────────────────────────────────────

  describe "GET /api/app/oauth/authorize_url" do
    before do
      allow(Google::OauthClient).to receive(:authorize_url)
        .and_return("https://accounts.google.com/o/oauth2/auth?fake=1")
      stub_const("Zoho::OauthClient::AUTH_URL", "https://accounts.zoho.com/oauth/v2/auth")
      stub_const("ENV", ENV.to_h.merge("ZOHO_CLIENT_ID" => "test_client_id"))
    end

    %w[google zoho].each do |provider|
      it "returns an authorize_url for #{provider}" do
        get "/api/app/oauth/authorize_url",
            params: { provider: provider },
            headers: api_app_headers(owner)

        expect(response).to have_http_status(:ok)
        data = response.parsed_body["data"]
        expect(data["provider"]).to eq(provider)
        expect(data["authorize_url"]).to be_present
      end
    end

    it "embeds spa: true, user_id, and return_to in the signed state" do
      state_captured = nil
      allow(Google::OauthClient).to receive(:authorize_url) do |redirect_uri:, state:, **|
        state_captured = state
        "https://accounts.google.com/o/oauth2/auth?state=#{state}"
      end

      get "/api/app/oauth/authorize_url",
          params: { provider: "google", return_to: "/settings/mailboxes" },
          headers: api_app_headers(owner)

      decoded = Oauth::State.decode(state_captured)
      expect(decoded["verified"]).to   be(true)
      expect(decoded["spa"]).to        be(true)
      expect(decoded["user_id"]).to    eq(owner.id)
      expect(decoded["flow"]).to       eq("account_link")
      expect(decoded["return_to"]).to  include("/settings/mailboxes")
    end

    it "returns 422 for an unknown provider" do
      get "/api/app/oauth/authorize_url",
          params: { provider: "unknown" },
          headers: api_app_headers(owner)

      expect(response).to have_http_status(:unprocessable_entity)
      expect(response.parsed_body.dig("error", "code")).to eq("invalid_provider")
    end

    it "returns 404 for microsoft when the feature is disabled" do
      allow(Features).to receive(:microsoft?).and_return(false)

      get "/api/app/oauth/authorize_url",
          params: { provider: "microsoft" },
          headers: api_app_headers(owner)

      expect(response).to have_http_status(:not_found)
      expect(response.parsed_body.dig("error", "code")).to eq("feature_disabled")
    end
  end

  # ── OAuth callback bridge — signed-state SPA path ────────────────────────────

  describe "OAuth callback bridge (SPA state)" do
    let(:frontend_url) { "http://localhost:3100" }

    let(:valid_state) do
      Oauth::State.encode(
        flow: "account_link",
        user_id: owner.id,
        spa: true,
        return_to: "#{frontend_url}/settings/mailboxes"
      )
    end

    let(:tampered_state) { "#{valid_state}tampered" }

    let(:expired_state) do
      Oauth::State.encode(
        flow: "account_link",
        user_id: owner.id,
        spa: true,
        return_to: "#{frontend_url}/settings/mailboxes",
        expires_in: -1.minute
      )
    end

    before do
      # Stub Google OAuth exchange + identity discovery for offline testing.
      fake_token_data = { "refresh_token" => "rt_test", "access_token" => "at_test" }
      fake_oauth = instance_double(Google::OauthClient, exchange_code: fake_token_data)
      allow(Google::OauthClient).to receive(:new).and_return(fake_oauth)

      allow(Google::AccountDiscovery).to receive(:new).and_return(
        instance_double(Google::AccountDiscovery,
                        discover_identity: {
                          email: "connected@example.com",
                          account_id: "ga_123",
                          name: "Test User"
                        })
      )

      # Silence side effects
      allow(Calendars::AccountProvisioner).to receive(:call)
      allow(Emails::WatchRenewalJob).to receive(:perform_later)
      allow(EmailScanJob).to receive(:perform_later)
      allow(Events).to receive(:publish)
    end

    it "creates an EmailAccount and redirects to return_to with status=success" do
      expect do
        get "/oauth/gmail/callback",
            params: { code: "auth_code", state: valid_state }
      end.to change(EmailAccount, :count).by(1)

      expect(response).to have_http_status(:redirect)
      expect(response.location).to include("settings/mailboxes")
      expect(response.location).to include("status=success")
    end

    it "binds the new account to the correct workspace" do
      get "/oauth/gmail/callback",
          params: { code: "auth_code", state: valid_state }

      expect(EmailAccount.last.workspace_id).to eq(workspace.id)
    end

    it "does not create an account when the state is tampered" do
      # A tampered state fails Oauth::State.decode verification; the callback
      # must NOT create an account and must NOT redirect to the SPA frontend.
      get "/oauth/gmail/callback",
          params: { code: "auth_code", state: tampered_state }

      expect(EmailAccount.count).to eq(0)
      expect(response.location).not_to include("localhost:3100") if response.redirect?
    end

    it "rejects an expired state" do
      get "/oauth/gmail/callback",
          params: { code: "auth_code", state: expired_state }

      expect(EmailAccount.count).to eq(0)
    end
  end

  # ── POST /api/app/imap_accounts ──────────────────────────────────────────────

  describe "POST /api/app/imap_accounts" do
    let(:imap_params) do
      {
        email_address: "user@example.com",
        imap_password: "secret",
        imap_host: "imap.example.com",
        imap_port: "993",
        imap_security: "ssl",
        smtp_host: "smtp.example.com",
        smtp_port: "587",
        smtp_security: "starttls"
      }
    end

    context "when IMAP is enabled" do
      before { allow(Features).to receive(:imap?).and_return(true) }

      it "returns 422 with authentication_failed when credentials are wrong" do
        allow_any_instance_of(Imap::MailClient).to receive(:verify!)
          .and_raise(AuthenticationError)
        # Skip host guard to isolate the credential check
        allow(Imap::HostGuard).to receive(:validate!)

        post "/api/app/imap_accounts",
             params: imap_params,
             headers: api_app_headers(owner)

        expect(response).to have_http_status(:unprocessable_entity)
        expect(response.parsed_body.dig("error", "code")).to eq("authentication_failed")
      end

      it "returns 422 with blocked_host when the host is blocked" do
        allow(Imap::HostGuard).to receive(:validate!)
          .and_raise(Imap::HostGuard::BlockedError, "Host is blocked")

        post "/api/app/imap_accounts",
             params: imap_params.merge(imap_host: "169.254.169.254"),
             headers: api_app_headers(owner)

        expect(response).to have_http_status(:unprocessable_entity)
        expect(response.parsed_body.dig("error", "code")).to eq("blocked_host")
      end

      it "creates an account and returns 201 when credentials verify" do
        allow(Imap::HostGuard).to receive(:validate!)
        allow_any_instance_of(Imap::MailClient).to receive(:verify!).and_return(true)
        allow(EmailScanJob).to receive(:perform_later)
        allow(Events).to receive(:publish)

        expect do
          post "/api/app/imap_accounts",
               params: imap_params,
               headers: api_app_headers(owner)
        end.to change(EmailAccount, :count).by(1)

        expect(response).to have_http_status(:created)
        data = response.parsed_body["data"]
        expect(data["email_address"]).to eq("user@example.com")
        expect(data["provider"]).to eq("imap")
      end
    end

    context "when IMAP is disabled" do
      before { allow(Features).to receive(:imap?).and_return(false) }

      it "returns 404" do
        post "/api/app/imap_accounts",
             params: imap_params,
             headers: api_app_headers(owner)

        expect(response).to have_http_status(:not_found)
      end
    end
  end
end

# rubocop:enable RSpec/MultipleMemoizedHelpers

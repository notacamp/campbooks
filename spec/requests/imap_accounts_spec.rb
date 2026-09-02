require "rails_helper"

RSpec.describe "ImapAccounts", type: :request do
  include ActiveJob::TestHelper

  let(:workspace) { create(:workspace) }
  let(:owner)     { create(:user, workspace: workspace) }
  let(:viewer)    { create(:user, workspace: workspace) }

  before do
    allow(Features).to receive(:imap?).and_return(true)
  end

  # ── Feature gate ─────────────────────────────────────────────────────────────

  describe "GET /imap_accounts/new" do
    it "returns 404 when IMAP is disabled (ENABLE_IMAP=0)" do
      allow(Features).to receive(:imap?).and_return(false)
      sign_in(owner)
      get new_imap_account_path
      expect(response).to have_http_status(:not_found)
    end

    it "renders 200 for authenticated users" do
      sign_in(owner)
      get new_imap_account_path
      expect(response).to have_http_status(:ok)
    end

    it "redirects unauthenticated visitors to login" do
      get new_imap_account_path
      expect(response).to redirect_to(new_session_path)
    end
  end

  # ── Create happy path ─────────────────────────────────────────────────────────

  describe "POST /imap_accounts" do
    let(:valid_params) do
      {
        email_account: {
          email_address:  "connect@example.com",
          imap_password:  "app-secret",
          imap_host:      "imap.example.com",
          imap_port:      993,
          imap_security:  "ssl",
          smtp_host:      "smtp.example.com",
          smtp_port:      587,
          smtp_security:  "starttls"
        }
      }
    end

    before do
      allow_any_instance_of(Imap::MailClient).to receive(:verify!).and_return(true)
      allow_any_instance_of(Entitlements::Resolver).to receive(:allow?)
        .with(:email_accounts).and_return(:ok)
      sign_in(owner)
    end

    it "creates the account with 90-day backfill, owner EmailAccountUser, and enqueues a scan" do
      expect {
        post imap_accounts_path, params: valid_params
      }.to change(EmailAccount, :count).by(1)
       .and have_enqueued_job(EmailScanJob).with(anything, "delta")

      account = EmailAccount.last
      expect(account.email_address).to eq("connect@example.com")
      expect(account.provider).to eq("imap")
      expect(account.backfill_since).to be_within(5.seconds).of(90.days.ago)

      eau = account.email_account_users.find_by(user: owner)
      expect(eau).not_to be_nil
      expect(eau.owner).to be(true)
      expect(eau.can_manage).to be(true)
      expect(eau.can_send).to be(true)

      expect(response).to redirect_to(email_messages_path(inbox_settings: "accounts"))
    end

    it "respects the sync_history param (30d, 1y, all)" do
      # UUID primary keys make .last ordering random — always look up by address.
      post imap_accounts_path, params: valid_params.merge(sync_history: "30d")
      expect(EmailAccount.find_by!(email_address: "connect@example.com").backfill_since)
        .to be_within(5.seconds).of(30.days.ago)

      post imap_accounts_path, params: valid_params.merge(
        email_account: valid_params[:email_account].merge(email_address: "b@example.com"),
        sync_history: "all"
      )
      expect(EmailAccount.find_by!(email_address: "b@example.com").backfill_since).to be_nil
    end
  end

  # ── Create: verify! failures ──────────────────────────────────────────────────

  describe "POST /imap_accounts with verify! failures" do
    let(:valid_params) do
      {
        email_account: {
          email_address: "fail@example.com",
          imap_password: "wrong",
          imap_host: "imap.example.com",
          imap_port: 993,
          imap_security: "ssl",
          smtp_host: "smtp.example.com",
          smtp_port: 587,
          smtp_security: "starttls"
        }
      }
    end

    before { sign_in(owner) }

    it "re-renders new (422) and does not create an account when verify! raises PermanentAuthError" do
      allow_any_instance_of(Imap::MailClient).to receive(:verify!).and_raise(PermanentAuthError, "bad creds")

      expect {
        post imap_accounts_path, params: valid_params
      }.not_to change(EmailAccount, :count)

      expect(response).to have_http_status(:unprocessable_entity)
    end

    it "re-renders new (422) for network errors (SocketError)" do
      allow_any_instance_of(Imap::MailClient).to receive(:verify!).and_raise(SocketError, "getaddrinfo failed")

      expect {
        post imap_accounts_path, params: valid_params
      }.not_to change(EmailAccount, :count)

      expect(response).to have_http_status(:unprocessable_entity)
    end
  end

  # ── Create: plan cap ─────────────────────────────────────────────────────────

  describe "POST /imap_accounts when the plan cap is reached" do
    before do
      allow_any_instance_of(Entitlements::Resolver).to receive(:allow?)
        .with(:email_accounts).and_return(:over_limit)
      allow_any_instance_of(Entitlements::Resolver).to receive(:limit)
        .with(:email_accounts).and_return(1)
      allow_any_instance_of(Entitlements::Resolver).to receive(:plan_name).and_return("free")
      sign_in(owner)
    end

    it "redirects without creating an account" do
      expect {
        post imap_accounts_path, params: {
          email_account: {
            email_address: "cap@example.com", imap_password: "pw",
            imap_host: "imap.example.com", imap_port: 993, imap_security: "ssl",
            smtp_host: "smtp.example.com", smtp_port: 587, smtp_security: "starttls"
          }
        }
      }.not_to change(EmailAccount, :count)

      expect(response).to have_http_status(:redirect)
    end
  end

  # ── Create: reconnect path ────────────────────────────────────────────────────

  describe "POST /imap_accounts (reconnect existing IMAP account)" do
    let!(:account) do
      create(:email_account, :imap, email_address: "reconnect@example.com", workspace: workspace)
    end

    before do
      create(:email_account_user, :owner, user: owner, email_account: account)
      allow_any_instance_of(Imap::MailClient).to receive(:verify!).and_return(true)
      # Cap is over limit — reconnect must bypass it.
      allow_any_instance_of(Entitlements::Resolver).to receive(:allow?)
        .with(:email_accounts).and_return(:over_limit)
      sign_in(owner)
    end

    it "updates the existing account and redirects without creating a duplicate" do
      expect {
        post imap_accounts_path, params: {
          email_account: {
            email_address:  "reconnect@example.com",
            imap_password:  "new-app-password",
            imap_host:      "imap.example.com",
            imap_port:      993,
            imap_security:  "ssl",
            smtp_host:      "smtp.example.com",
            smtp_port:      587,
            smtp_security:  "starttls"
          }
        }
      }.not_to change(EmailAccount, :count)

      expect(account.reload.active).to be(true)
      expect(response).to redirect_to(email_messages_path(inbox_settings: "accounts"))
    end
  end

  # ── Create: address hijack guard ─────────────────────────────────────────────

  describe "POST /imap_accounts for an address whose account the caller cannot manage" do
    # verify! runs against whatever host the FORM supplies, so it proves nothing
    # about mailbox ownership — without the management check this request would
    # repoint the victim's account at the attacker's server and grant the
    # attacker an owner entry over the victim's already-synced mail.
    let(:other_workspace) { create(:workspace) }
    let!(:victim_account) do
      create(:email_account, :imap, email_address: "victim@example.com", workspace: other_workspace)
    end

    before do
      allow_any_instance_of(Imap::MailClient).to receive(:verify!).and_return(true)
      allow_any_instance_of(Entitlements::Resolver).to receive(:allow?)
        .with(:email_accounts).and_return(:ok)
      sign_in(owner)
    end

    it "refuses, leaves the victim's settings untouched, and grants no access" do
      original_host = victim_account.imap_host

      expect {
        post imap_accounts_path, params: {
          email_account: {
            email_address:  "victim@example.com",
            imap_password:  "whatever",
            imap_host:      "attacker.example.com",
            imap_port:      993,
            imap_security:  "ssl",
            smtp_host:      "attacker.example.com",
            smtp_port:      587,
            smtp_security:  "starttls"
          }
        }
      }.not_to change(EmailAccount, :count)

      expect(response).to have_http_status(:unprocessable_entity)
      expect(victim_account.reload.imap_host).to eq(original_host)
      expect(victim_account.email_account_users.where(user: owner)).to be_empty
    end
  end

  # ── Update (edit/update) ──────────────────────────────────────────────────────

  describe "PATCH /imap_accounts/:id" do
    let!(:account) { create(:email_account, :imap, workspace: workspace) }

    before do
      create(:email_account_user, :owner, user: owner, email_account: account)
      allow_any_instance_of(Imap::MailClient).to receive(:verify!).and_return(true)
    end

    it "denies a non-manager and redirects to the accounts panel" do
      create(:email_account_user, :viewer, user: viewer, email_account: account)
      sign_in(viewer)

      patch imap_account_path(account), params: {
        email_account: {
          imap_host: "new.imap.example.com", imap_port: 993, imap_security: "ssl",
          smtp_host: "new.smtp.example.com", smtp_port: 587, smtp_security: "starttls",
          imap_password: ""
        }
      }

      expect(response).to redirect_to(email_messages_path(inbox_settings: "accounts"))
      # Account is unchanged
      expect(account.reload.imap_host).to eq("imap.example.com")
    end

    it "keeps the old password when imap_password param is blank" do
      old_password = account.imap_password
      sign_in(owner)

      patch imap_account_path(account), params: {
        email_account: {
          imap_host: "updated.imap.example.com", imap_port: 993, imap_security: "ssl",
          smtp_host: "updated.smtp.example.com", smtp_port: 587, smtp_security: "starttls",
          imap_password: ""
        }
      }

      expect(response).to redirect_to(email_messages_path(inbox_settings: "accounts"))
      account.reload
      expect(account.imap_password).to eq(old_password)
      expect(account.imap_host).to eq("updated.imap.example.com")
    end

    it "updates the password when a new value is provided" do
      sign_in(owner)

      patch imap_account_path(account), params: {
        email_account: {
          imap_host: "imap.example.com", imap_port: 993, imap_security: "ssl",
          smtp_host: "smtp.example.com", smtp_port: 587, smtp_security: "starttls",
          imap_password: "brand-new-password"
        }
      }

      expect(response).to redirect_to(email_messages_path(inbox_settings: "accounts"))
      expect(account.reload.imap_password).to eq("brand-new-password")
    end

    it "returns 404 for a non-IMAP account" do
      other = create(:email_account, workspace: workspace)
      create(:email_account_user, :owner, user: owner, email_account: other)
      sign_in(owner)

      patch imap_account_path(other), params: {
        email_account: { imap_host: "x", imap_port: 993, imap_security: "ssl",
                         smtp_host: "x", smtp_port: 587, smtp_security: "starttls" }
      }

      expect(response).to have_http_status(:not_found)
    end
  end

  # ── GET /imap_accounts/:id/edit ───────────────────────────────────────────────

  describe "GET /imap_accounts/:id/edit" do
    let!(:account) { create(:email_account, :imap, workspace: workspace) }

    before { create(:email_account_user, :owner, user: owner, email_account: account) }

    it "renders 200 for the account manager" do
      sign_in(owner)
      get edit_imap_account_path(account)
      expect(response).to have_http_status(:ok)
    end

    it "returns 404 when the feature is disabled" do
      allow(Features).to receive(:imap?).and_return(false)
      sign_in(owner)
      get edit_imap_account_path(account)
      expect(response).to have_http_status(:not_found)
    end
  end
end

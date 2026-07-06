require "rails_helper"

RSpec.describe Accounts::Remover do
  # Suppress broadcast side-effects from notification callbacks, and stub the
  # OAuth client so token revocation never fires a real HTTP request.
  before do
    allow_any_instance_of(Notification).to receive(:broadcast_replace_to)
    allow_any_instance_of(Notification).to receive(:broadcast_remove_to)
    allow_any_instance_of(Notification).to receive(:broadcast_append_to)
    allow_any_instance_of(EmailAccount).to receive(:oauth_client).and_return(double("oauth_client"))
  end

  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  before { create(:email_account_user, :owner, email_account: account, user: user) }

  describe "#remove! — mail data teardown" do
    let!(:scan_log) { create(:email_scan_log, email_account: account) }
    let!(:message) { create(:email_message, email_account: account, email_scan_log: scan_log) }
    let!(:thread) { create(:email_thread, email_account: account) }
    let!(:folder) { create(:email_folder, email_account: account) }
    let!(:scheduled) { create(:scheduled_email, workspace: workspace, email_account: account, created_by: user) }

    it "destroys the account and everything mail-specific" do
      described_class.new(account).remove!

      expect { account.reload }.to raise_error(ActiveRecord::RecordNotFound)
      expect { message.reload }.to raise_error(ActiveRecord::RecordNotFound)
      expect { scan_log.reload }.to raise_error(ActiveRecord::RecordNotFound)
      expect { thread.reload }.to raise_error(ActiveRecord::RecordNotFound)
      expect { folder.reload }.to raise_error(ActiveRecord::RecordNotFound)
      expect { scheduled.reload }.to raise_error(ActiveRecord::RecordNotFound)
    end

    it "does not raise on the restrict_with_error associations (messages/scan logs)" do
      expect { described_class.new(account).remove! }.not_to raise_error
    end
  end

  describe "#remove! — keeps derived documents and contacts (detached)" do
    let!(:document) { create(:document, workspace: workspace, email_account: account) }
    let!(:contact)  { create(:contact, email_account: account) }

    it "keeps the document but nulls its link to the removed mailbox" do
      described_class.new(account).remove!

      expect { document.reload }.not_to raise_error
      expect(document.reload.email_account_id).to be_nil
    end

    it "keeps the contact but nulls its link to the removed mailbox" do
      described_class.new(account).remove!

      expect { contact.reload }.not_to raise_error
      expect(contact.reload.email_account_id).to be_nil
    end
  end

  describe "#remove! — best-effort OAuth token revocation" do
    it "revokes the grant when no sibling shares the token" do
      fake_client = double("oauth_client")
      allow(fake_client).to receive(:respond_to?).with(:revoke_token).and_return(true)
      allow(account).to receive(:oauth_client).and_return(fake_client)

      expect(fake_client).to receive(:revoke_token)
      described_class.new(account).remove!
    end

    it "does not revoke when a still-connected calendar account shares the token" do
      account.update!(refresh_token: "shared-token")
      create(:calendar_account, workspace: workspace, refresh_token: "shared-token")

      fake_client = double("oauth_client")
      allow(fake_client).to receive(:respond_to?).with(:revoke_token).and_return(true)
      allow(account).to receive(:oauth_client).and_return(fake_client)

      expect(fake_client).not_to receive(:revoke_token)
      described_class.new(account).remove!
    end

    it "does not raise when the provider revoke fails" do
      fake_client = double("oauth_client")
      allow(fake_client).to receive(:respond_to?).with(:revoke_token).and_return(true)
      allow(fake_client).to receive(:revoke_token).and_raise(StandardError, "network error")
      allow(account).to receive(:oauth_client).and_return(fake_client)

      expect { described_class.new(account).remove! }.not_to raise_error
      expect { account.reload }.to raise_error(ActiveRecord::RecordNotFound)
    end
  end

  describe EmailAccountRemovalJob do
    it "runs the remover for a live account" do
      expect(Accounts::Remover).to receive(:new).with(account).and_call_original
      described_class.perform_now(account.id)
    end

    it "is a no-op when the account is already gone (idempotent)" do
      expect(Accounts::Remover).not_to receive(:new)
      expect { described_class.perform_now("00000000-0000-0000-0000-000000000000") }.not_to raise_error
    end
  end
end

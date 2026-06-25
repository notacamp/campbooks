require "rails_helper"

RSpec.describe Accounts::Deleter do
  # Suppress broadcast side-effects from notifications/callbacks.
  before do
    allow_any_instance_of(Notification).to receive(:broadcast_replace_to)
    allow_any_instance_of(Notification).to receive(:broadcast_remove_to)
    allow_any_instance_of(Notification).to receive(:broadcast_append_to)

    # WebMock isn't wired globally, so a real oauth_client would fire an actual
    # HTTP revoke against Google/Zoho during deletion. Default to a harmless
    # double (no #revoke_token, so revoke_tokens_best_effort skips it); the
    # revocation context below overrides this per-example with its own spy.
    allow_any_instance_of(EmailAccount).to receive(:oauth_client).and_return(double("oauth_client"))
    allow_any_instance_of(CalendarAccount).to receive(:oauth_client).and_return(double("oauth_client"))
  end

  describe "#delete! — sole workspace member (destroys everything)" do
    let(:workspace) { create(:workspace) }
    let(:user) { create(:user, workspace: workspace) }

    let!(:email_account) { create(:email_account, workspace: workspace) }
    let!(:email_account_user) { create(:email_account_user, :owner, email_account: email_account, user: user) }
    let!(:email_message) { create(:email_message, email_account: email_account) }
    let!(:email_scan_log) { create(:email_scan_log, email_account: email_account) }

    let!(:calendar_account) { create(:calendar_account, workspace: workspace) }
    let!(:calendar_account_user) { create(:calendar_account_user, :owner, calendar_account: calendar_account, user: user) }
    let!(:calendar) { create(:calendar, calendar_account: calendar_account) }
    let!(:calendar_event) { create(:calendar_event, calendar: calendar) }
    let!(:calendar_sync_log) { create(:calendar_sync_log, calendar_account: calendar_account) }

    let!(:document) { create(:document, workspace: workspace, reviewed_by_id: user.id) }
    let!(:workflow) { create(:workflow, workspace: workspace, created_by_id: user.id) }
    let!(:contact) { create(:contact, workspace: workspace, email_account: nil) }

    it "destroys the user, workspace, and all content without error" do
      expect { described_class.new(user).delete! }.not_to raise_error

      expect { user.reload }.to raise_error(ActiveRecord::RecordNotFound)
      expect { workspace.reload }.to raise_error(ActiveRecord::RecordNotFound)
      expect { email_account.reload }.to raise_error(ActiveRecord::RecordNotFound)
      expect { calendar_account.reload }.to raise_error(ActiveRecord::RecordNotFound)
      expect { document.reload }.to raise_error(ActiveRecord::RecordNotFound)
      expect { contact.reload }.to raise_error(ActiveRecord::RecordNotFound)
    end

    it "nullifies reviewed_by_id and created_by_id before destroying so no FK violation" do
      # These FK landmines must be nullified in the sole-member path too
      expect { described_class.new(user).delete! }.not_to raise_error
    end
  end

  describe "#delete! — one of several workspace members (removes only this user)" do
    let(:workspace) { create(:workspace) }
    let(:admin_user) { create(:user, workspace: workspace, role: :admin) }
    let(:leaving_user) { create(:user, workspace: workspace) }

    let!(:email_account) { create(:email_account, workspace: workspace) }
    let!(:calendar_account) { create(:calendar_account, workspace: workspace) }

    before do
      # leaving_user solely owns the email account and calendar account
      create(:email_account_user, :owner, email_account: email_account, user: leaving_user)
      create(:email_account_user, :viewer, email_account: email_account, user: admin_user)
      create(:calendar_account_user, :owner, calendar_account: calendar_account, user: leaving_user)
      create(:calendar_account_user, :viewer, calendar_account: calendar_account, user: admin_user)
    end

    it "keeps the workspace and shared accounts intact" do
      described_class.new(leaving_user).delete!

      expect { workspace.reload }.not_to raise_error
      expect { email_account.reload }.not_to raise_error
      expect { calendar_account.reload }.not_to raise_error
    end

    it "destroys only the leaving user's personal rows" do
      expect { described_class.new(leaving_user).delete! }.not_to raise_error
      expect { leaving_user.reload }.to raise_error(ActiveRecord::RecordNotFound)
    end

    it "promotes a co-member to owner on the email account when leaving user was sole owner" do
      described_class.new(leaving_user).delete!

      new_owner = EmailAccountUser.find_by(email_account: email_account, user: admin_user)
      expect(new_owner).to be_present
      expect(new_owner.owner).to be(true)
    end

    it "promotes a co-member to owner on the calendar account when leaving user was sole owner" do
      described_class.new(leaving_user).delete!

      new_owner = CalendarAccountUser.find_by(calendar_account: calendar_account, user: admin_user)
      expect(new_owner).to be_present
      expect(new_owner.owner).to be(true)
    end

    it "nullifies reviewed_by_id on documents belonging to this user before destroying" do
      document = create(:document, workspace: workspace, reviewed_by_id: leaving_user.id)
      described_class.new(leaving_user).delete!
      expect(document.reload.reviewed_by_id).to be_nil
    end

    it "nullifies created_by_id on workflows belonging to this user before destroying" do
      workflow = create(:workflow, workspace: workspace, created_by_id: leaving_user.id)
      described_class.new(leaving_user).delete!
      expect(workflow.reload.created_by_id).to be_nil
    end
  end

  describe "#delete! — best-effort OAuth token revocation" do
    let(:workspace) { create(:workspace) }
    let(:user) { create(:user, workspace: workspace) }

    it "calls revoke_token on an oauth_client that supports it" do
      create(:email_account, workspace: workspace)

      fake_client = double("oauth_client", respond_to?: false)
      allow(fake_client).to receive(:respond_to?).with(:revoke_token).and_return(true)
      allow(fake_client).to receive(:revoke_token).and_return(true)
      allow_any_instance_of(EmailAccount).to receive(:oauth_client).and_return(fake_client)
      allow_any_instance_of(CalendarAccount).to receive(:oauth_client).and_return(fake_client)

      expect(fake_client).to receive(:revoke_token).at_least(:once)
      described_class.new(user).delete!
    end

    it "does not raise when the oauth_client raises StandardError during revoke" do
      create(:email_account, workspace: workspace)

      fake_client = double("oauth_client", respond_to?: false)
      allow(fake_client).to receive(:respond_to?).with(:revoke_token).and_return(true)
      allow(fake_client).to receive(:revoke_token).and_raise(StandardError, "network error")
      allow_any_instance_of(EmailAccount).to receive(:oauth_client).and_return(fake_client)
      allow_any_instance_of(CalendarAccount).to receive(:oauth_client).and_return(fake_client)

      expect { described_class.new(user).delete! }.not_to raise_error
    end

    it "revokes the Google Drive grant at Google" do
      GoogleDriveAccount.create!(workspace: workspace, refresh_token: "drive-token", connected: true, email: "d@example.com")

      drive_client = instance_double(GoogleDrive::OauthClient)
      allow(GoogleDrive::OauthClient).to receive(:new).and_return(drive_client)
      expect(drive_client).to receive(:revoke_token).with("drive-token").and_return(true)

      described_class.new(user).delete!
    end

    it "does not raise when the Google Drive revoke fails" do
      GoogleDriveAccount.create!(workspace: workspace, refresh_token: "drive-token", connected: true, email: "d@example.com")
      allow_any_instance_of(GoogleDrive::OauthClient).to receive(:revoke_token).and_raise(StandardError, "boom")

      expect { described_class.new(user).delete! }.not_to raise_error
    end

    it "does not raise for a Notion integration (no revoke API), and drops it" do
      NotionIntegration.create!(workspace: workspace, access_token: "notion-token", active: true)

      expect { described_class.new(user).delete! }.not_to raise_error
      expect(NotionIntegration.where(workspace_id: workspace.id)).to be_empty
    end
  end

  describe "#delete! — attached blob purging" do
    let(:workspace) { create(:workspace) }
    let(:user) { create(:user, workspace: workspace) }

    it "calls purge_later on attached document original_file before deleting records" do
      document = create(:document, workspace: workspace)
      expect(document.original_file).to be_attached

      purged_ids = []
      # Track purge_later calls by intercepting ActiveStorage::Attached::One#purge_later
      allow_any_instance_of(ActiveStorage::Attached::One).to receive(:purge_later) do |attached|
        purged_ids << attached.record.id if attached.attached?
        nil
      end

      described_class.new(user).delete!

      expect(purged_ids).to include(document.id)
    end
  end
end

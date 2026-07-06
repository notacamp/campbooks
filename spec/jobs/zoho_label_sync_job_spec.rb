require "rails_helper"

RSpec.describe ZohoLabelSyncJob, type: :job do
  it "isolates a per-account failure and reports it instead of aborting the batch" do
    workspace = create(:workspace)
    failing = create(:email_account, workspace: workspace, provider: :google)
    healthy = create(:email_account, workspace: workspace, provider: :google)
    allow(EmailAccount).to receive(:active).and_return([ failing, healthy ])

    bad  = instance_double(Google::LabelSyncService)
    good = instance_double(Google::LabelSyncService)
    allow(Google::LabelSyncService).to receive(:new).with(failing).and_return(bad)
    allow(Google::LabelSyncService).to receive(:new).with(healthy).and_return(good)
    allow(bad).to receive(:sync_labels!).and_raise(StandardError, "boom")
    allow(good).to receive(:sync_labels!).and_return(3)

    # The failure is surfaced to error tracking, not swallowed …
    expect(Rails.error).to receive(:report)
      .with(instance_of(StandardError), hash_including(context: hash_including(account_id: failing.id)))

    described_class.perform_now

    # … and the batch continues past the bad mailbox to the healthy one.
    expect(good).to have_received(:sync_labels!)
  end

  # Converted from test/jobs/zoho_label_sync_job_test.rb — MailboxUnavailable
  # handling and provider routing.
  describe "Emails::MailboxUnavailable handling (Google account with no Gmail mailbox)" do
    let(:workspace) { create(:workspace) }
    let(:account) { create(:email_account, workspace: workspace, provider: :google) }
    let(:service_double) { instance_double(Google::LabelSyncService) }

    before do
      allow(Google::LabelSyncService).to receive(:new).with(account).and_return(service_double)
      allow(service_double).to receive(:sync_labels!)
        .and_raise(Emails::MailboxUnavailable, "Gmail is not enabled for this Google account")
    end

    it "deactivates the account with mail_service_unavailable and does not re-raise" do
      expect { described_class.perform_now(account.id) }.not_to raise_error
      account.reload
      expect(account.active?).to be(false)
      expect(account.deactivation_reason).to eq("mail_service_unavailable")
      expect(account.deactivated_for_service?).to be(true)
    end

    it "does not re-raise Emails::MailboxUnavailable out of the job" do
      expect { described_class.perform_now(account.id) }.not_to raise_error
    end

    context "when the account is already inactive" do
      before { account.deactivate_for!(:mail_service_unavailable) }

      it "is skipped by the active scope so sync_labels! is never called" do
        described_class.perform_now(account.id)
        expect(service_double).not_to have_received(:sync_labels!)
      end
    end
  end

  describe "provider routing" do
    let(:workspace) { create(:workspace) }

    it "routes a Zoho account to Zoho::LabelSyncService, not Google::LabelSyncService" do
      zoho_account = create(:email_account, workspace: workspace, provider: :zoho)
      zoho_service = instance_double(Zoho::LabelSyncService, sync_labels!: 0)

      allow(Zoho::LabelSyncService).to receive(:new).with(zoho_account).and_return(zoho_service)
      allow(Google::LabelSyncService).to receive(:new)

      described_class.perform_now(zoho_account.id)

      expect(Zoho::LabelSyncService).to have_received(:new).with(zoho_account)
      expect(Google::LabelSyncService).not_to have_received(:new)
      expect(zoho_service).to have_received(:sync_labels!)
    end
  end
end

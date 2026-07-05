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
end

require "rails_helper"

RSpec.describe Emails::FullResyncJob, type: :job do
  let(:account) { create(:email_account) }

  before { ActiveJob::Base.queue_adapter.enqueued_jobs.clear }

  it "hands off to EmailScanJob in full mode for the account" do
    expect { described_class.perform_now(account.id) }
      .to have_enqueued_job(EmailScanJob).with(account.id, "full")
  end

  it "rate-limits repeat resyncs within the window" do
    # Test env uses :null_store, so pin a real store to exercise the rate limit.
    allow(Rails).to receive(:cache).and_return(ActiveSupport::Cache::MemoryStore.new)

    described_class.perform_now(account.id)
    ActiveJob::Base.queue_adapter.enqueued_jobs.clear

    expect { described_class.perform_now(account.id) }.not_to have_enqueued_job(EmailScanJob)
  end

  it "ignores an inactive account" do
    account.deactivate!
    expect { described_class.perform_now(account.id) }.not_to have_enqueued_job(EmailScanJob)
  end

  it "ignores a missing account" do
    expect { described_class.perform_now(-1) }.not_to have_enqueued_job(EmailScanJob)
  end
end

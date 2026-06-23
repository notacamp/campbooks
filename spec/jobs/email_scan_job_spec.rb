require "rails_helper"

# EmailScanJob is the provider-agnostic engine: it owns the slot-lock, the scan
# log, and the live "syncing" pill, and delegates the actual fetch to the account's
# Emails::SyncStrategies strategy. These specs pin the engine's orchestration with
# the strategy stubbed; per-vendor fetch behaviour lives in the strategy specs.
RSpec.describe EmailScanJob, type: :job do
  let(:account) { create(:email_account) }
  let(:result) { Emails::SyncStrategies::Result.new(found: 2, created: 1, reconciled: 1) }
  let(:strategy) do
    instance_double(
      Emails::SyncStrategies::Zoho,
      needs_bootstrap?: false,
      needs_periodic_resync?: true,
      sync!: result,
      full_resync!: result
    )
  end

  before do
    ActiveJob::Base.queue_adapter.enqueued_jobs.clear
    allow(Emails::SyncStrategies).to receive(:for).and_return(strategy)
  end

  describe "delta mode (default, every-minute poll)" do
    it "runs the strategy's incremental sync and records the counts" do
      expect { described_class.perform_now(account.id) }.to change(EmailScanLog, :count).by(1)

      expect(strategy).to have_received(:sync!)
      log = EmailScanLog.last
      expect(log.status).to eq("completed")
      expect(log.emails_found).to eq(2)
      expect(log.emails_processed).to eq(1)
    end

    it "claims and releases the scan slot" do
      described_class.perform_now(account.id)
      expect(account.reload.scanning).to be false
      expect(account.last_scanned_at).to be_present
    end
  end

  describe "bootstrap offload (never baselined)" do
    before { allow(strategy).to receive(:needs_bootstrap?).and_return(true) }

    it "enqueues a full resync instead of walking inline, without claiming the slot or scanning" do
      expect { described_class.perform_now(account.id) }
        .to have_enqueued_job(Emails::FullResyncJob).with(account.id)

      expect(strategy).not_to have_received(:sync!)
      expect(EmailScanLog.count).to eq(0)
      expect(account.reload.scanning).to be false
    end
  end

  describe "cursor expiry recovery" do
    before { allow(strategy).to receive(:sync!).and_raise(Emails::CursorExpired) }

    it "enqueues a full resync, closes the log cleanly (not a failure), and releases the slot" do
      expect { described_class.perform_now(account.id) }
        .to have_enqueued_job(Emails::FullResyncJob).with(account.id)

      expect(EmailScanLog.last.status).to eq("completed")
      expect(account.reload.scanning).to be false
    end
  end

  describe "full mode" do
    it "runs the strategy's full resync" do
      described_class.perform_now(account.id, "full")
      expect(strategy).to have_received(:full_resync!)
      expect(strategy).not_to have_received(:sync!)
      expect(EmailScanLog.last.status).to eq("completed")
    end

    it "does not offload to a resync even if the strategy reports needs_bootstrap" do
      allow(strategy).to receive(:needs_bootstrap?).and_return(true)
      described_class.perform_now(account.id, "full")
      expect(strategy).to have_received(:full_resync!)
    end
  end

  describe "resync_sweep mode (periodic read/flag reconcile)" do
    it "enqueues a full run for a vendor that needs a periodic reconcile (Zoho)" do
      expect { described_class.perform_now(account.id, "resync_sweep") }
        .to have_enqueued_job(EmailScanJob).with(account.id, "full")
    end

    it "skips vendors whose change feed already carries read/flag changes" do
      allow(strategy).to receive(:needs_periodic_resync?).and_return(false)
      expect { described_class.perform_now(account.id, "resync_sweep") }
        .not_to have_enqueued_job(EmailScanJob)
    end
  end

  describe "when the strategy raises a generic error" do
    before { allow(strategy).to receive(:sync!).and_raise(StandardError.new("API error")) }

    it "marks the scan log failed and releases the slot" do
      described_class.perform_now(account.id)
      log = EmailScanLog.last
      expect(log.status).to eq("failed")
      expect(log.error_messages).to be_present
      expect(account.reload.scanning).to be false
    end
  end

  describe "inactive account" do
    it "is skipped entirely" do
      account.deactivate!
      described_class.perform_now(account.id)
      expect(strategy).not_to have_received(:sync!)
      expect(EmailScanLog.count).to eq(0)
    end
  end

  # The sync pill renders Campbooks::SyncIndicator for every user on the account.
  # The bare :email_account factory has no users, so pin one to exercise the real
  # view-context render that once crashed prod (`undefined method 't' for nil`).
  describe "live sync pill" do
    it "renders the pill without crashing when the account has a user" do
      create(:email_account_user, :owner, email_account: account,
                                           user: create(:user, workspace: account.workspace))

      expect { described_class.perform_now(account.id) }.not_to raise_error
      expect(EmailScanLog.last.status).to eq("completed")
    end
  end

  # The scanning flag is the live "syncing" pill's source of truth, so a leaked
  # flag strands the pill. Preserved from the perma-loader fix — independent of the
  # delta rework.
  describe "stale scan reconciliation" do
    it "clears a scanning flag stranded by a dead worker so the pill stops sticking" do
      account.update_columns(scanning: true, scan_started_at: (EmailAccount::SCAN_STALE_AFTER + 1.minute).ago)
      allow_any_instance_of(described_class).to receive(:claim_scan_slot).and_return(false)

      described_class.perform_now(account.id)
      expect(account.reload.scanning).to be false
    end

    it "leaves a fresh scanning flag alone (does not interrupt a scan in flight)" do
      account.update_columns(scanning: true, scan_started_at: 30.seconds.ago)
      allow_any_instance_of(described_class).to receive(:claim_scan_slot).and_return(false)

      described_class.perform_now(account.id)
      expect(account.reload.scanning).to be true
    end
  end
end

require "rails_helper"

RSpec.describe RetentionSweepJob, type: :job do
  let(:workspace) { create(:workspace) }
  let(:account) { create(:email_account, workspace: workspace) }
  let(:calendar_account) { create(:calendar_account, workspace: workspace) }

  it "prunes operational logs past the retention window, keeps recent ones" do
    old_scan = create(:email_scan_log, email_account: account)
    old_scan.update_column(:created_at, (described_class::LOG_RETENTION + 10.days).ago)
    recent_scan = create(:email_scan_log, email_account: account)

    old_sync = create(:calendar_sync_log, calendar_account: calendar_account)
    old_sync.update_column(:created_at, (described_class::LOG_RETENTION + 10.days).ago)

    described_class.new.perform

    expect(EmailScanLog.exists?(old_scan.id)).to be(false)
    expect(EmailScanLog.exists?(recent_scan.id)).to be(true)
    expect(CalendarSyncLog.exists?(old_sync.id)).to be(false)
  end

  it "prunes domain events past the retention window, keeps recent ones" do
    old_event = create(:event, workspace: workspace, occurred_at: (described_class::LOG_RETENTION + 10.days).ago)
    recent_event = create(:event, workspace: workspace, occurred_at: 1.day.ago)

    described_class.new.perform

    expect(Event.exists?(old_event.id)).to be(false)
    expect(Event.exists?(recent_event.id)).to be(true)
  end

  it "never deletes user content (emails survive the sweep)" do
    recent = create(:email_scan_log, email_account: account)
    message = create(:email_message, email_account: account)

    described_class.new.perform

    expect(EmailScanLog.exists?(recent.id)).to be(true)
    expect(EmailMessage.exists?(message.id)).to be(true)
  end
end

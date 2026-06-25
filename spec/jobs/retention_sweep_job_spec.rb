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

  it "prunes audit events past the retention window, keeps recent ones" do
    user = create(:user, workspace: workspace)
    old_event = AuditEvent.create!(user: user, action: "sign_in")
    old_event.update_column(:created_at, (described_class::AUDIT_EVENT_RETENTION + 1.month).ago)
    recent_event = AuditEvent.create!(user: user, action: "sign_in")

    described_class.new.perform

    expect(AuditEvent.exists?(old_event.id)).to be(false)
    expect(AuditEvent.exists?(recent_event.id)).to be(true)
  end

  describe "opt-in content retention" do
    it "deletes email older than the window for an opted-in workspace, keeps recent" do
      workspace.update!(email_retention_months: 12)
      old_mail = create(:email_message, email_account: account, received_at: 18.months.ago)
      recent_mail = create(:email_message, email_account: account, received_at: 1.month.ago)

      described_class.new.perform

      expect(EmailMessage.exists?(old_mail.id)).to be(false)
      expect(EmailMessage.exists?(recent_mail.id)).to be(true)
    end

    it "keeps all email for a workspace that hasn't opted in (NULL retention)" do
      workspace.update!(email_retention_months: nil)
      old_mail = create(:email_message, email_account: account, received_at: 5.years.ago)

      described_class.new.perform

      expect(EmailMessage.exists?(old_mail.id)).to be(true)
    end

    it "removes the derived search index for deleted email" do
      workspace.update!(email_retention_months: 6)
      old_mail = create(:email_message, email_account: account, received_at: 2.years.ago)
      chunk = SearchChunk.create!(workspace: workspace, searchable: old_mail, content: "body text")

      described_class.new.perform

      expect(SearchChunk.exists?(chunk.id)).to be(false)
    end

    it "deletes only OUR copy — the mailbox connection (EmailAccount) is untouched" do
      workspace.update!(email_retention_months: 12)
      create(:email_message, email_account: account, received_at: 3.years.ago)

      described_class.new.perform

      # The account row (and its OAuth connection) survives — we never delete or
      # disconnect the provider mailbox, only the local message copy.
      expect(EmailAccount.exists?(account.id)).to be(true)
    end
  end
end

require "rails_helper"

RSpec.describe Reminders::Confirm do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }

  def with_writable_calendar
    account = create(:calendar_account, workspace: workspace)
    create(:calendar_account_user, :editor, user: user, calendar_account: account)
    create(:calendar, :primary, calendar_account: account, is_writable: true, syncing: true)
  end

  context "with a writable calendar" do
    before { with_writable_calendar }

    it "creates a calendar event, links it, and enqueues the push" do
      reminder = create(:reminder, workspace: workspace)

      result = nil
      expect { result = described_class.call(reminder, user: user) }
        .to change(CalendarEvent, :count).by(1)
        .and have_enqueued_job(Calendars::EventWriteJob)

      expect(result.success?).to be(true)
      expect(result.calendar?).to be(true)
      expect(reminder.reload).to be_confirmed
      expect(reminder.calendar_event).to eq(CalendarEvent.last)
    end

    it "links an email-sourced reminder back to its source message" do
      email = create(:email_message)
      reminder = create(:reminder, workspace: workspace, source: email)
      described_class.call(reminder, user: user)
      expect(CalendarEvent.last.source_email_message).to eq(email)
    end

    it "leaves source_email_message nil for a document-sourced reminder" do
      reminder = create(:reminder, workspace: workspace, source: create(:document, workspace: workspace))
      described_class.call(reminder, user: user)
      expect(CalendarEvent.last.source_email_message).to be_nil
    end
  end

  context "without a writable calendar" do
    it "still confirms the reminder, creating no event" do
      reminder = create(:reminder, workspace: workspace)

      result = nil
      expect { result = described_class.call(reminder, user: user) }.not_to change(CalendarEvent, :count)

      expect(result.success?).to be(true)
      expect(result.calendar?).to be(false)
      expect(reminder.reload).to be_confirmed
      expect(reminder.calendar_event).to be_nil
    end
  end

  context "when the reminder's source mailbox has its own calendar" do
    it "routes the event to that mailbox's calendar, not the user's primary" do
      # The user's generic primary calendar (a different account).
      primary_account = create(:calendar_account, workspace: workspace)
      create(:calendar_account_user, :editor, user: user, calendar_account: primary_account)
      create(:calendar, :primary, calendar_account: primary_account, is_writable: true, syncing: true)

      # The calendar tied to the mailbox that produced the reminder.
      src_account = create(:calendar_account, workspace: workspace, email_address: "shared@x.com", provider: :google)
      create(:calendar_account_user, :editor, user: user, calendar_account: src_account)
      src_cal = create(:calendar, calendar_account: src_account, is_writable: true, syncing: true)

      email_account = create(:email_account, workspace: workspace, email_address: "shared@x.com", provider: :google)
      reminder = create(:reminder, workspace: workspace, source: create(:email_message, email_account: email_account))

      described_class.call(reminder, user: user)

      expect(reminder.reload.calendar_event.calendar).to eq(src_cal)
    end
  end
end

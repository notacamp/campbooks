require "rails_helper"

RSpec.describe Tools::CreateCalendarEvent do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:email)     { create(:email_message) }

  def with_writable_calendar
    account = create(:calendar_account, workspace: workspace)
    create(:calendar_account_user, :editor, user: user, calendar_account: account)
    create(:calendar, :primary, calendar_account: account, is_writable: true, syncing: true)
  end

  context "with a writable calendar" do
    before { with_writable_calendar }

    it "creates an event from the email and enqueues the provider push" do
      result = nil
      expect { result = described_class.call(email, { title: "Project sync" }, user: user) }
        .to change(CalendarEvent, :count).by(1)
        .and have_enqueued_job(Calendars::EventWriteJob)

      expect(result).to be_a(CalendarEvent)
      expect(result.source_email_message).to eq(email)
      expect(result.title).to eq("Project sync")
    end

    it "returns the existing event instead of creating a duplicate for the same email" do
      first = described_class.call(email, { title: "Project sync" }, user: user)

      second = nil
      expect { second = described_class.call(email, { title: "Phrased differently" }, user: user) }
        .not_to change(CalendarEvent, :count)
      expect(second).to eq(first)
    end

    it "still creates a second event when an explicit start time is on a different day" do
      day1 = 1.day.from_now.change(hour: 10)
      day5 = 5.days.from_now.change(hour: 10)
      described_class.call(email, { title: "Meeting", start_time: day1.iso8601 }, user: user)

      expect { described_class.call(email, { title: "Follow-up", start_time: day5.iso8601 }, user: user) }
        .to change(CalendarEvent, :count).by(1)
    end

    it "confirms and links a same-day pending reminder from the same email" do
      start_time = 2.days.from_now.change(hour: 9)
      reminder = create(:reminder, workspace: workspace, source: email, due_at: start_time)

      described_class.call(email, { title: "Meeting", start_time: start_time.iso8601 }, user: user)

      expect(reminder.reload).to be_confirmed
      expect(reminder.calendar_event).to eq(CalendarEvent.last)
      expect(reminder.confirmed_by).to eq(user)
    end
  end

  context "without a writable calendar" do
    it "returns nil and creates nothing" do
      result = nil
      expect { result = described_class.call(email, { title: "X" }, user: user) }
        .not_to change(CalendarEvent, :count)
      expect(result).to be_nil
    end
  end
end

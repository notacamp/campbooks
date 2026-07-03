require "rails_helper"

# The shared TimeUntil countdown, exercised through the two agenda rows that use
# it. Every branch is driven through ReminderChip (:row) — it renders from an
# in-memory Reminder and links to the /reminders collection, so no persisted id
# is needed — plus one persisted-event check that EventRow surfaces the label too.
RSpec.describe Campbooks::Calendar::TimeUntil, type: :component do
  include ActiveSupport::Testing::TimeHelpers

  # Freeze mid-month, mid-morning so day/hour maths never straddle a boundary.
  around { |example| travel_to(Time.zone.local(2026, 7, 15, 9, 0, 0)) { example.run } }

  def reminder_html(due_at:, all_day: false)
    reminder = Reminder.new(id: 1, title: "Pay invoice", reminder_type: :payment_due, status: :pending,
                            due_at: due_at, all_day: all_day)
    ApplicationController.render(Campbooks::Calendar::ReminderChip.new(reminder: reminder, variant: :row), layout: false)
  end

  describe "countdown text (via ReminderChip :row)" do
    it "minutes for a timed item within the hour" do
      expect(reminder_html(due_at: 45.minutes.from_now)).to include("In 45 min")
    end

    it "hours later the same day" do
      expect(reminder_html(due_at: Time.current.change(hour: 17))).to include("In 8 h")
    end

    it "Tomorrow for a timed item tomorrow" do
      expect(reminder_html(due_at: 1.day.from_now.change(hour: 10))).to include("Tomorrow")
    end

    it "days for an all-day item a few days out" do
      expect(reminder_html(due_at: 4.days.from_now.beginning_of_day, all_day: true)).to include("In 4 days")
    end

    it "Today for an all-day item today" do
      expect(reminder_html(due_at: Time.current.beginning_of_day, all_day: true)).to include("Today")
    end

    it "Next week between one and two weeks out" do
      expect(reminder_html(due_at: 9.days.from_now.beginning_of_day, all_day: true)).to include("Next week")
    end

    it "weeks for two-plus weeks out" do
      expect(reminder_html(due_at: 20.days.from_now.beginning_of_day, all_day: true)).to include("In 3 weeks")
    end

    it "months for a month-plus out" do
      expect(reminder_html(due_at: 60.days.from_now.beginning_of_day, all_day: true)).to include("In 2 months")
    end
  end

  describe "imminent accent" do
    it "accents an item within the hour" do
      expect(reminder_html(due_at: 20.minutes.from_now)).to include("text-accent-700")
    end

    it "does not accent an item days away" do
      expect(reminder_html(due_at: 4.days.from_now.beginning_of_day, all_day: true)).not_to include("text-accent-700")
    end
  end

  describe "EventRow" do
    it "shows the countdown next to a persisted event" do
      event = create(:calendar_event, start_at: 45.minutes.from_now, end_at: 105.minutes.from_now, all_day: false)
      html = ApplicationController.render(Campbooks::Calendar::EventRow.new(event: event), layout: false)
      expect(html).to include("In 45 min")
    end
  end
end

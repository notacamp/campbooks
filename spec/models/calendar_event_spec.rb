require "rails_helper"

RSpec.describe CalendarEvent, type: :model do
  describe "enums" do
    it {
      is_expected.to define_enum_for(:status)
        .with_values(confirmed: 0, tentative: 1, cancelled: 2)
    }

    it {
      is_expected.to define_enum_for(:rsvp_status)
        .with_values(needs_action: 0, accepted: 1, declined: 2, tentative: 3)
        .with_prefix(:rsvp)
    }
  end

  describe "validations" do
    subject { build(:calendar_event) }

    it { is_expected.to validate_presence_of(:provider_event_id) }
    it { is_expected.to validate_uniqueness_of(:provider_event_id).scoped_to(:calendar_id) }
  end

  describe "#recurring?" do
    it "returns false when there is no recurring series id" do
      event = build(:calendar_event, recurring_event_provider_id: nil)
      expect(event.recurring?).to be(false)
    end

    it "returns true when there is a recurring series id" do
      event = build(:calendar_event, :recurring)
      expect(event.recurring?).to be(true)
    end
  end

  describe "#display_color" do
    let(:account) { create(:calendar_account, color: "#0584da") }
    let(:calendar) { create(:calendar, calendar_account: account, color: nil) }

    it "returns the event's own color when set" do
      event = build(:calendar_event, calendar: calendar, color: "#dc2127")
      expect(event.display_color).to eq("#dc2127")
    end

    it "falls back to the calendar's color when unset" do
      event = build(:calendar_event, calendar: calendar, color: nil)
      expect(event.display_color).to eq(calendar.display_color)
    end

    it "treats a blank color as unset" do
      event = build(:calendar_event, calendar: calendar, color: "")
      expect(event.display_color).to eq(calendar.display_color)
    end
  end

  describe "scopes" do
    let(:workspace) { create(:workspace) }
    let(:account) { create(:calendar_account, workspace: workspace) }
    let(:calendar) { create(:calendar, calendar_account: account) }

    describe ".visible" do
      it "returns confirmed and tentative events but not cancelled ones" do
        confirmed = create(:calendar_event, calendar: calendar, status: :confirmed)
        tentative = create(:calendar_event, :tentative, calendar: calendar)
        cancelled = create(:calendar_event, :cancelled, calendar: calendar)

        result = CalendarEvent.visible
        expect(result).to include(confirmed, tentative)
        expect(result).not_to include(cancelled)
      end
    end

    describe ".upcoming" do
      it "returns future visible events ordered by start_at" do
        future_a = create(:calendar_event, calendar: calendar, start_at: 2.days.from_now, end_at: 2.days.from_now + 1.hour)
        future_b = create(:calendar_event, calendar: calendar, start_at: 1.day.from_now, end_at: 1.day.from_now + 1.hour)
        past = create(:calendar_event, calendar: calendar, start_at: 1.day.ago, end_at: 1.day.ago + 1.hour)
        cancelled_future = create(:calendar_event, :cancelled, calendar: calendar, start_at: 3.days.from_now, end_at: 3.days.from_now + 1.hour)

        result = CalendarEvent.upcoming
        expect(result.to_a).to eq([ future_b, future_a ])
        expect(result).not_to include(past, cancelled_future)
      end
    end

    describe ".in_range" do
      it "returns events overlapping the given window" do
        window_start = Time.current
        window_end = 3.days.from_now

        # Fully inside
        inside = create(:calendar_event, calendar: calendar,
          start_at: 1.day.from_now, end_at: 1.day.from_now + 2.hours)
        # Starts before window but ends inside
        straddle_start = create(:calendar_event, calendar: calendar,
          start_at: 1.hour.ago, end_at: 1.hour.from_now)
        # Starts inside, ends after
        straddle_end = create(:calendar_event, calendar: calendar,
          start_at: 2.days.from_now, end_at: 4.days.from_now)
        # Entirely before window
        before_window = create(:calendar_event, calendar: calendar,
          start_at: 2.days.ago, end_at: 1.day.ago)
        # Entirely after window
        after_window = create(:calendar_event, calendar: calendar,
          start_at: 4.days.from_now, end_at: 5.days.from_now)

        result = CalendarEvent.in_range(window_start, window_end)
        expect(result).to include(inside, straddle_start, straddle_end)
        expect(result).not_to include(before_window, after_window)
      end
    end
  end

  describe ".accessible_to" do
    let(:workspace) { create(:workspace) }
    let(:user) { create(:user, workspace: workspace) }
    let(:readable_account) { create(:calendar_account, workspace: workspace) }
    let(:other_account) { create(:calendar_account, workspace: workspace) }
    let(:readable_calendar) { create(:calendar, calendar_account: readable_account) }
    let(:other_calendar) { create(:calendar, calendar_account: other_account) }
    let!(:visible_event) { create(:calendar_event, calendar: readable_calendar) }
    let!(:hidden_event) { create(:calendar_event, calendar: other_calendar) }

    before { create(:calendar_account_user, :viewer, user: user, calendar_account: readable_account) }

    it "returns only events on calendars whose account the user can read" do
      expect(CalendarEvent.accessible_to(user)).to contain_exactly(visible_event)
    end

    it "excludes an account shared without read access" do
      create(:calendar_account_user, user: user, calendar_account: other_account, can_read: false)
      expect(CalendarEvent.accessible_to(user)).to contain_exactly(visible_event)
    end

    it "fails closed for a nil user" do
      expect(CalendarEvent.accessible_to(nil)).to be_empty
    end
  end
end

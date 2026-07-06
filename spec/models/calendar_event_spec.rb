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

    it "always returns the owning calendar's display color" do
      event = build(:calendar_event, calendar: calendar)
      expect(event.display_color).to eq(calendar.display_color)
    end

    it "reflects the calendar's own color when it has one" do
      calendar.update!(color: "#dc2127")
      event = build(:calendar_event, calendar: calendar)
      expect(event.display_color).to eq("#dc2127")
    end

    it "falls through to the account color when the calendar has none" do
      event = build(:calendar_event, calendar: calendar)
      expect(event.display_color).to eq("#0584da")
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

  # ── From CalendarEventTest (Minitest migration) ──────────────────────────────

  describe "display_color (direct creation)" do
    before do
      @ws      = Workspace.create!(name: "CalEvent Test WS")
      account  = @ws.calendar_accounts.create!(email_address: "cal@example.com", refresh_token: "tok")
      @calendar = account.calendars.create!(provider_calendar_id: "pc1", name: "Primary", color: "#123456")
      @event   = @calendar.calendar_events.create!(
        provider_event_id: "e1", title: "X",
        start_at: Time.current, end_at: Time.current + 1.hour
      )
    end

    it "display_color is always the owning calendar's color" do
      expect(@event.display_color).to eq("#123456")
    end

    it "an assigned event type never changes the color (only its icon marks the event)" do
      type = @ws.event_types.create!(name: "Meeting", icon: "users")
      @event.update!(event_type: type)
      expect(@event.display_color).to eq("#123456")
      expect(@event.event_type.icon).to eq("users")
    end

    it "a calendar without its own color falls back to the account color" do
      @event.calendar.update!(color: nil)
      expect(@event.display_color).to eq(@event.calendar_account.color)
    end

    it "type_status defaults to pending" do
      expect(@event).to be_type_status_pending
    end
  end

  # ── From CalendarEventRecurrenceTest (Minitest migration) ────────────────────

  describe "recurrence predicates" do
    it "recurring? is true for a local/Zoho master (rrule only)" do
      event = CalendarEvent.new(rrule: "FREQ=WEEKLY")
      expect(event).to be_recurring
      expect(event).to be_series_master
      expect(event).not_to be_series_instance
    end

    it "recurring? is true for a provider-materialized instance (series id only)" do
      event = CalendarEvent.new(recurring_event_provider_id: "series-1")
      expect(event).to be_recurring
      expect(event).not_to be_series_master
      expect(event).to be_series_instance
    end

    it "a plain event is neither master nor instance" do
      event = CalendarEvent.new
      expect(event).not_to be_recurring
      expect(event).not_to be_series_master
      expect(event).not_to be_series_instance
    end

    it "a blank rrule normalizes to nil so master/instance queries stay NULL-clean" do
      event = CalendarEvent.new(rrule: "")
      event.valid? # triggers before_validation
      expect(event.rrule).to be_nil
      expect(event).not_to be_series_master
    end

    it "an unparseable rrule is rejected" do
      event = CalendarEvent.new(provider_event_id: "x", rrule: "not a rule")
      expect(event).not_to be_valid
      expect(event.errors[:rrule]).to be_present
    end

    it "recurrence exposes the value object" do
      expect(CalendarEvent.new(rrule: "FREQ=WEEKLY").recurrence.preset_key).to eq(:weekly)
    end
  end

  describe ".duplicate_for" do
    let(:calendar) { create(:calendar) }
    let(:email)    { create(:email_message) }

    it "returns a non-cancelled event sourced from the email" do
      event = create(:calendar_event, calendar: calendar, source_email_message: email)
      expect(CalendarEvent.duplicate_for(email: email)).to eq(event)
    end

    it "ignores cancelled events" do
      create(:calendar_event, :cancelled, calendar: calendar, source_email_message: email)
      expect(CalendarEvent.duplicate_for(email: email)).to be_nil
    end

    it "returns nil for a nil email" do
      expect(CalendarEvent.duplicate_for(email: nil)).to be_nil
    end

    it "returns nil when the email has no event" do
      expect(CalendarEvent.duplicate_for(email: email)).to be_nil
    end

    it "matches only same-day events when a start_at is given" do
      day1 = 2.days.from_now.change(hour: 10)
      same_day = create(:calendar_event, calendar: calendar, source_email_message: email,
                        start_at: day1, end_at: day1 + 1.hour)
      other = 9.days.from_now.change(hour: 10)
      create(:calendar_event, calendar: calendar, source_email_message: email,
             start_at: other, end_at: other + 1.hour)

      expect(CalendarEvent.duplicate_for(email: email, start_at: day1)).to eq(same_day)
    end

    it "returns the oldest event when several match" do
      older = create(:calendar_event, calendar: calendar, source_email_message: email, created_at: 2.days.ago)
      create(:calendar_event, calendar: calendar, source_email_message: email, created_at: 1.day.ago)
      expect(CalendarEvent.duplicate_for(email: email)).to eq(older)
    end
  end
end

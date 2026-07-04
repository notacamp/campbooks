require "test_helper"

class CalendarEventRecurrenceTest < ActiveSupport::TestCase
  test "recurring? is true for a local/Zoho master (rrule only)" do
    event = CalendarEvent.new(rrule: "FREQ=WEEKLY")
    assert event.recurring?
    assert event.series_master?
    assert_not event.series_instance?
  end

  test "recurring? is true for a provider-materialized instance (series id only)" do
    event = CalendarEvent.new(recurring_event_provider_id: "series-1")
    assert event.recurring?
    assert_not event.series_master?
    assert event.series_instance?
  end

  test "a plain event is neither master nor instance" do
    event = CalendarEvent.new
    assert_not event.recurring?
    assert_not event.series_master?
    assert_not event.series_instance?
  end

  test "a blank rrule normalizes to nil so master/instance queries stay NULL-clean" do
    event = CalendarEvent.new(rrule: "")
    event.valid? # triggers before_validation
    assert_nil event.rrule
    assert_not event.series_master?
  end

  test "an unparseable rrule is rejected" do
    event = CalendarEvent.new(provider_event_id: "x", rrule: "not a rule")
    assert_not event.valid?
    assert event.errors[:rrule].present?
  end

  test "recurrence exposes the value object" do
    assert_equal :weekly, CalendarEvent.new(rrule: "FREQ=WEEKLY").recurrence.preset_key
  end
end

require "test_helper"

class CalendarEventTest < ActiveSupport::TestCase
  setup do
    @ws = Workspace.create!(name: "CalEvent Test WS")
    account = @ws.calendar_accounts.create!(email_address: "cal@example.com", refresh_token: "tok")
    @calendar = account.calendars.create!(provider_calendar_id: "pc1", name: "Primary", color: "#123456")
    @event = @calendar.calendar_events.create!(
      provider_event_id: "e1", title: "X",
      start_at: Time.current, end_at: Time.current + 1.hour
    )
  end

  test "display_color falls back to the calendar color when untyped" do
    assert_equal "#123456", @event.display_color
  end

  test "provider_color is nil when untyped and unoverridden (inherit at provider)" do
    assert_nil @event.provider_color
  end

  test "an assigned event type colors the event" do
    type = @ws.event_types.create!(name: "Meeting", color: "#ff0000")
    @event.update!(event_type: type)
    assert_equal "#ff0000", @event.display_color
    assert_equal "#ff0000", @event.provider_color
  end

  test "an explicit per-event color overrides the type color" do
    type = @ws.event_types.create!(name: "Meeting", color: "#ff0000")
    @event.update!(event_type: type, color: "#00ff00")
    assert_equal "#00ff00", @event.display_color
    assert_equal "#00ff00", @event.provider_color
  end

  test "type_status defaults to pending" do
    assert @event.type_status_pending?
  end
end

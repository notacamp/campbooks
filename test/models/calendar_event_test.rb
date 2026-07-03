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

  test "display_color is always the owning calendar's color" do
    assert_equal "#123456", @event.display_color
  end

  test "an assigned event type never changes the color (only its icon marks the event)" do
    type = @ws.event_types.create!(name: "Meeting", icon: "users")
    @event.update!(event_type: type)
    assert_equal "#123456", @event.display_color
    assert_equal "users", @event.event_type.icon
  end

  test "a calendar without its own color falls back to the account color" do
    @event.calendar.update!(color: nil)
    assert_equal @event.calendar_account.color, @event.display_color
  end

  test "type_status defaults to pending" do
    assert @event.type_status_pending?
  end
end

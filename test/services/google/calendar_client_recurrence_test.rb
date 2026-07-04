require "test_helper"

# build_payload is a pure function of its attrs hash, so we exercise it directly
# (allocate skips the connection the initializer would build).
class Google::CalendarClientRecurrenceTest < ActiveSupport::TestCase
  setup { @client = Google::CalendarClient.allocate }

  def payload(attrs)
    @client.send(:build_payload, attrs)
  end

  test "a recurring event sends the RRULE as a recurrence line" do
    result = payload(
      title: "Standup",
      start_at: Time.utc(2026, 7, 6, 9, 0),
      end_at: Time.utc(2026, 7, 6, 9, 30),
      all_day: false,
      time_zone: "UTC",
      rrule: "FREQ=WEEKLY"
    )
    assert_equal [ "RRULE:FREQ=WEEKLY" ], result[:recurrence]
  end

  test "a one-off event carries no recurrence key" do
    result = payload(title: "Lunch", start_at: Time.utc(2026, 7, 6, 12, 0), all_day: false, time_zone: "UTC")
    assert_not result.key?(:recurrence)
  end
end

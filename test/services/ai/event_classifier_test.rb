require "test_helper"

class Ai::EventClassifierTest < ActiveSupport::TestCase
  setup do
    @ws = Workspace.create!(name: "Classifier Test WS")
    account = @ws.calendar_accounts.create!(email_address: "cal@example.com", refresh_token: "tok")
    calendar = account.calendars.create!(provider_calendar_id: "pc1", name: "Primary")
    @event = calendar.calendar_events.create!(
      provider_event_id: "e1", title: "Sync with Acme",
      start_at: Time.current, end_at: Time.current + 1.hour
    )
  end

  # An EventClassifier whose model call is stubbed to return the given raw text.
  def classifier_returning(text)
    Ai::EventClassifier.new(@event).tap do |c|
      c.define_singleton_method(:generate_text) { |*| text }
    end
  end

  test "returns nil when the workspace has no event types" do
    assert_nil classifier_returning('{"type":"Meeting"}').call
  end

  test "maps the chosen name back to the event type" do
    meeting = @ws.event_types.create!(name: "Meeting", color: "#5484ed")
    assert_equal meeting, classifier_returning('{"type":"Meeting"}').call
  end

  test "matching is case-insensitive" do
    meeting = @ws.event_types.create!(name: "Meeting", color: "#5484ed")
    assert_equal meeting, classifier_returning('{"type":"meeting"}').call
  end

  test "returns nil when the AI declines to pick a type" do
    @ws.event_types.create!(name: "Meeting", color: "#5484ed")
    assert_nil classifier_returning('{"type":null}').call
  end

  test "tolerates a fenced ```json code block" do
    meeting = @ws.event_types.create!(name: "Meeting", color: "#5484ed")
    assert_equal meeting, classifier_returning("```json\n{\"type\":\"Meeting\"}\n```").call
  end

  test "returns nil on unparseable model output" do
    @ws.event_types.create!(name: "Meeting", color: "#5484ed")
    assert_nil classifier_returning("not json at all").call
  end
end

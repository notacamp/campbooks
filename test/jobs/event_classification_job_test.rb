require "test_helper"

class EventClassificationJobTest < ActiveSupport::TestCase
  setup do
    @ws = Workspace.create!(name: "Classify Job WS")
    account = @ws.calendar_accounts.create!(email_address: "cal@example.com", refresh_token: "tok")
    calendar = account.calendars.create!(provider_calendar_id: "pc1", name: "Primary")
    @event = calendar.calendar_events.create!(
      provider_event_id: "local-e1", title: "Sync",
      start_at: Time.current, end_at: Time.current + 1.hour
    )
    @type = @ws.event_types.create!(name: "Meeting", icon: "users")
  end

  # minitest 6 ships no Object#stub, so save/restore the methods by hand.
  def stub_configured(value)
    original = Ai::ProviderSetup.method(:configured?)
    Ai::ProviderSetup.define_singleton_method(:configured?) { |*| value }
    yield
  ensure
    Ai::ProviderSetup.define_singleton_method(:configured?, original)
  end

  def stub_classifier_result(value)
    original = Ai::EventClassifier.instance_method(:call)
    Ai::EventClassifier.define_method(:call) { value }
    yield
  ensure
    Ai::EventClassifier.define_method(:call, original)
  end

  test "assigns the classified type and marks it auto" do
    stub_configured(true) do
      stub_classifier_result(@type) { EventClassificationJob.new.perform(@event.id) }
    end
    @event.reload
    assert_equal @type, @event.event_type
    assert @event.type_status_auto?
  end

  test "marks auto even when no type matches, so it never re-runs" do
    stub_configured(true) do
      stub_classifier_result(nil) { EventClassificationJob.new.perform(@event.id) }
    end
    @event.reload
    assert_nil @event.event_type
    assert @event.type_status_auto?
  end

  test "skips when no text AI is configured (stays pending)" do
    stub_configured(false) { EventClassificationJob.new.perform(@event.id) }
    @event.reload
    assert_nil @event.event_type
    assert @event.type_status_pending?
  end

  test "never overwrites a manually-typed event" do
    @event.update!(type_status: :manual)
    # No stubs needed: the pending-guard returns before any AI work.
    EventClassificationJob.new.perform(@event.id)
    @event.reload
    assert @event.type_status_manual?
    assert_nil @event.event_type
  end
end

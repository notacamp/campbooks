require "rails_helper"

RSpec.describe EventClassificationJob, type: :job do
  let(:ws) { Workspace.create!(name: "Classify Job WS") }
  let(:account) { ws.calendar_accounts.create!(email_address: "cal@example.com", refresh_token: "tok") }
  let(:calendar) { account.calendars.create!(provider_calendar_id: "pc1", name: "Primary") }
  let(:event) do
    calendar.calendar_events.create!(
      provider_event_id: "local-e1", title: "Sync",
      start_at: Time.current, end_at: Time.current + 1.hour
    )
  end
  let(:type) { ws.event_types.create!(name: "Meeting", icon: "users") }

  # Save/restore the real class methods to avoid dependency on minitest stubs.
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

  it "assigns the classified type and marks it auto" do
    stub_configured(true) do
      stub_classifier_result(type) { described_class.new.perform(event.id) }
    end
    event.reload
    expect(event.event_type).to eq(type)
    expect(event.type_status_auto?).to be true
  end

  it "marks auto even when no type matches, so it never re-runs" do
    stub_configured(true) do
      stub_classifier_result(nil) { described_class.new.perform(event.id) }
    end
    event.reload
    expect(event.event_type).to be_nil
    expect(event.type_status_auto?).to be true
  end

  it "skips when no text AI is configured (stays pending)" do
    stub_configured(false) { described_class.new.perform(event.id) }
    event.reload
    expect(event.event_type).to be_nil
    expect(event.type_status_pending?).to be true
  end

  it "never overwrites a manually-typed event" do
    event.update!(type_status: :manual)
    # No stubs needed: the pending-guard returns before any AI work.
    described_class.new.perform(event.id)
    event.reload
    expect(event.type_status_manual?).to be true
    expect(event.event_type).to be_nil
  end
end

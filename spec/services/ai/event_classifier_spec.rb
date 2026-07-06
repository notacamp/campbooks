require "rails_helper"

RSpec.describe Ai::EventClassifier do
  before do
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
    described_class.new(@event).tap do |c|
      allow(c).to receive(:generate_text).and_return(text)
    end
  end

  it "returns nil when the workspace has no event types" do
    expect(classifier_returning('{"type":"Meeting"}').call).to be_nil
  end

  it "maps the chosen name back to the event type" do
    meeting = @ws.event_types.create!(name: "Meeting", icon: "users")
    expect(classifier_returning('{"type":"Meeting"}').call).to eq(meeting)
  end

  it "matching is case-insensitive" do
    meeting = @ws.event_types.create!(name: "Meeting", icon: "users")
    expect(classifier_returning('{"type":"meeting"}').call).to eq(meeting)
  end

  it "returns nil when the AI declines to pick a type" do
    @ws.event_types.create!(name: "Meeting", icon: "users")
    expect(classifier_returning('{"type":null}').call).to be_nil
  end

  it "tolerates a fenced ```json code block" do
    meeting = @ws.event_types.create!(name: "Meeting", icon: "users")
    expect(classifier_returning("```json\n{\"type\":\"Meeting\"}\n```").call).to eq(meeting)
  end

  it "returns nil on unparseable model output" do
    @ws.event_types.create!(name: "Meeting", icon: "users")
    expect(classifier_returning("not json at all").call).to be_nil
  end
end

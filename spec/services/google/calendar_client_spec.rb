require "rails_helper"

# Tests for the provider-not-enabled detection paths in Google::CalendarClient.
# Uses WebMock to simulate the API responses; allocates the client directly so
# no OAuth HTTP call is needed.
# Converted from test/services/google/calendar_client_test.rb.
RSpec.describe Google::CalendarClient, "outbound attendee payloads" do
  let(:client) do
    c = described_class.allocate
    # Allocate without calling initialize — @oauth is injected manually so the
    # token-refresh POST to accounts.google.com is never made.
    fake_oauth = double("oauth", access_token: "fake_access_token")
    c.instance_variable_set(:@oauth, fake_oauth)
    c
  end
  let(:calendar) { build(:calendar, provider_calendar_id: "cal_1@group.calendar.google.com") }
  let(:events_url) { "https://www.googleapis.com/calendar/v3/calendars/#{CGI.escape(calendar.provider_calendar_id)}/events" }

  before { WebMock.disable_net_connect! }

  it "maps attendees from any stored shape, keeping names and response statuses" do
    captured = nil
    stub_request(:post, events_url)
      .with(query: { "sendUpdates" => "all" }) { |req| captured = JSON.parse(req.body); true }
      .to_return(status: 200, body: "{}", headers: { "Content-Type" => "application/json" })

    client.create_event(calendar, {
      title: "Kickoff",
      start_at: Time.utc(2026, 7, 16, 14, 0),
      end_at: Time.utc(2026, 7, 16, 15, 0),
      attendees: [
        { email: "maya@example.com", name: "Maya", rsvp_status: "accepted" },   # canonical (our enum)
        { "email" => "rui@example.com", "rsvp_status" => "needsAction" },        # raw jsonb row (Google vocab)
        { email: "sam@example.com", rsvp_status: "needs_action" },               # our enum needing mapping
        "bare@example.com",                                                      # bare string
        { "name" => "No Email" }                                                 # dropped, Google 400s on it
      ]
    })

    expect(captured["attendees"]).to eq([
      { "email" => "maya@example.com", "displayName" => "Maya", "responseStatus" => "accepted" },
      { "email" => "rui@example.com", "responseStatus" => "needsAction" },
      { "email" => "sam@example.com", "responseStatus" => "needsAction" },
      { "email" => "bare@example.com" }
    ])
  end

  it "sends the full mapped list on an RSVP patch" do
    captured = nil
    stub_request(:patch, "#{events_url}/evt_9")
      .with(query: { "sendUpdates" => "all" }) { |req| captured = JSON.parse(req.body); true }
      .to_return(status: 200, body: "{}", headers: { "Content-Type" => "application/json" })

    client.patch_rsvp(calendar, "evt_9", attendees: [
      { email: "organizer@example.com", rsvp_status: "accepted" },
      { email: "me@example.com", rsvp_status: "tentative" }
    ])

    expect(captured["attendees"]).to eq([
      { "email" => "organizer@example.com", "responseStatus" => "accepted" },
      { "email" => "me@example.com", "responseStatus" => "tentative" }
    ])
  end
end

RSpec.describe Google::CalendarClient, "ServiceUnavailable detection" do
  let(:calendar_list_url) { "https://www.googleapis.com/calendar/v3/users/me/calendarList" }

  let(:client) do
    c = described_class.allocate
    # Allocate without calling initialize — @oauth is injected manually so the
    # token-refresh POST to accounts.google.com is never made.
    fake_oauth = double("oauth", access_token: "fake_access_token")
    c.instance_variable_set(:@oauth, fake_oauth)
    c
  end

  before { WebMock.disable_net_connect! }

  context "when the provider returns 403 with 'must be signed up' message" do
    before do
      stub_request(:get, calendar_list_url)
        .to_return(
          status: 403,
          body: '{"error":{"code":403,"message":"The user must be signed up for Google Calendar."}}',
          headers: { "Content-Type" => "application/json" }
        )
    end

    it "raises Calendars::ServiceUnavailable" do
      expect { client.calendar_list }.to raise_error(Calendars::ServiceUnavailable)
    end
  end

  context "when the provider returns a generic 403 (different message)" do
    before do
      stub_request(:get, calendar_list_url)
        .to_return(
          status: 403,
          body: '{"error":{"code":403,"message":"The caller does not have permission"}}',
          headers: { "Content-Type" => "application/json" }
        )
    end

    it "raises a plain RuntimeError, not Calendars::ServiceUnavailable" do
      expect { client.calendar_list }.to raise_error(RuntimeError) do |err|
        expect(err).not_to be_a(Calendars::ServiceUnavailable)
      end
    end
  end

  context "when the provider returns 401" do
    before do
      stub_request(:get, calendar_list_url)
        .to_return(
          status: 401,
          body: '{"error":{"code":401,"message":"Invalid Credentials"}}',
          headers: { "Content-Type" => "application/json" }
        )
    end

    it "raises AuthenticationError (not Calendars::ServiceUnavailable)" do
      expect { client.calendar_list }.to raise_error(AuthenticationError)
    end
  end

  context "when the provider returns a successful empty response" do
    before do
      stub_request(:get, calendar_list_url)
        .to_return(
          status: 200,
          body: "{}",
          headers: { "Content-Type" => "application/json" }
        )
    end

    it "returns an empty array" do
      expect(client.calendar_list).to eq([])
    end
  end
end

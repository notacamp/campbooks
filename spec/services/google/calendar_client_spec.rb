require "rails_helper"

# Tests for the provider-not-enabled detection paths in Google::CalendarClient.
# Uses WebMock to simulate the API responses; allocates the client directly so
# no OAuth HTTP call is needed.
# Converted from test/services/google/calendar_client_test.rb.
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

require "rails_helper"

RSpec.describe Zoho::CalendarClient, type: :service do
  let(:account) { build(:calendar_account, provider: :zoho, provider_account_id: "ACC123") }
  let(:oauth) { instance_double(Zoho::OauthClient, access_token: "fake-token") }
  let(:conn) { instance_double(Faraday::Connection) }
  let(:client) { described_class.new(account) }
  let(:calendar) { build(:calendar, provider_calendar_id: "cal_001", calendar_account: account) }

  let(:base_url) { "https://calendar.zoho.eu/api/v1" }

  before do
    allow(Zoho::OauthClient).to receive(:new)
                                  .with(refresh_token: account.refresh_token)
                                  .and_return(oauth)
    allow(client).to receive(:connection).and_return(conn)
  end

  # ---------------------------------------------------------------------------
  # Helpers
  # ---------------------------------------------------------------------------

  def ok_response(body_hash)
    instance_double(Faraday::Response, body: body_hash.to_json, success?: true, status: 200)
  end

  def error_response(status, body = {})
    instance_double(Faraday::Response, body: body.to_json, success?: false, status: status)
  end

  def fake_req
    headers = {}
    params = {}
    body_store = nil
    req = double("faraday_request")
    allow(req).to receive(:headers).and_return(headers)
    allow(req).to receive(:params).and_return(params)
    allow(req).to receive(:body=) { |v| body_store = v }
    allow(req).to receive(:body) { body_store }
    req
  end

  # ---------------------------------------------------------------------------
  # Fix 2 - list_events_full 31-day chunking
  # ---------------------------------------------------------------------------

  describe "#list_events_full" do
    context "when the window spans more than 30 days" do
      it "issues multiple requests each covering at most 30 days" do
        time_min = Time.utc(2026, 1, 1)
        time_max = Time.utc(2026, 3, 1) # 59 days => 2 slices

        event1 = { "uid" => "evt_a", "title" => "Slice 1 event", "dateandtime" => {} }
        event2 = { "uid" => "evt_b", "title" => "Slice 2 event", "dateandtime" => {} }

        # Both slices share the same calendar URL but different range params.
        # We rely on call order: first call returns slice 1, second returns slice 2.
        allow(conn).to receive(:get)
                         .with("#{base_url}/calendars/cal_001/events", anything)
                         .and_return(
                           ok_response("events" => [ event1 ]),
                           ok_response("events" => [ event2 ])
                         )

        result = client.list_events_full(calendar, time_min: time_min, time_max: time_max)

        expect(conn).to have_received(:get)
                          .with("#{base_url}/calendars/cal_001/events", anything)
                          .exactly(2).times
        expect(result[:events].map { |e| e[:provider_event_id] }).to contain_exactly("evt_a", "evt_b")
        expect(result[:next_sync_token]).to be_nil
      end

      it "deduplicates events that appear in both slices" do
        time_min = Time.utc(2026, 1, 1)
        time_max = Time.utc(2026, 3, 1) # 59 days => 2 slices

        shared_event = { "uid" => "evt_shared", "title" => "Spans boundary", "dateandtime" => {} }
        unique_event = { "uid" => "evt_unique", "title" => "Only in slice 2", "dateandtime" => {} }

        allow(conn).to receive(:get)
                         .with("#{base_url}/calendars/cal_001/events", anything)
                         .and_return(
                           ok_response("events" => [ shared_event ]),
                           ok_response("events" => [ shared_event, unique_event ])
                         )

        result = client.list_events_full(calendar, time_min: time_min, time_max: time_max)

        ids = result[:events].map { |e| e[:provider_event_id] }
        expect(ids).to contain_exactly("evt_shared", "evt_unique")
        expect(ids.tally.values).to all(eq(1)) # no duplicates
      end

      it "aborts and returns empty events when a slice request fails" do
        time_min = Time.utc(2026, 1, 1)
        time_max = Time.utc(2026, 3, 1)

        allow(conn).to receive(:get)
                         .with("#{base_url}/calendars/cal_001/events", anything)
                         .and_return(error_response(400, "error" => "RANGE_CANNOT_EXCEED_31DAYS"))

        result = client.list_events_full(calendar, time_min: time_min, time_max: time_max)

        expect(result).to eq({ events: [], next_sync_token: nil })
      end
    end

    context "when the window fits in a single 30-day slice" do
      it "issues exactly one request" do
        time_min = Time.utc(2026, 1, 1)
        time_max = Time.utc(2026, 1, 15) # 14 days => 1 slice

        allow(conn).to receive(:get)
                         .with("#{base_url}/calendars/cal_001/events", anything)
                         .and_return(ok_response("events" => []))

        client.list_events_full(calendar, time_min: time_min, time_max: time_max)

        expect(conn).to have_received(:get).exactly(1).times
      end
    end
  end

  # ---------------------------------------------------------------------------
  # Fix 3 - build_payload strips nil values
  # ---------------------------------------------------------------------------

  describe "#build_payload (via create_event)" do
    let(:req_double) { fake_req }

    before do
      # Stub create_event to capture the body sent to the API
      allow(conn).to receive(:post)
                       .with("#{base_url}/calendars/cal_001/events")
                       .and_yield(req_double)
                       .and_return(ok_response("events" => [ { "uid" => "new_evt", "title" => "Meeting", "dateandtime" => {} } ]))
    end

    it "omits description when it is nil" do
      client.create_event(calendar, { title: "Meeting", description: nil,
                                      start_at: Time.utc(2026, 6, 1, 10) })
      payload = JSON.parse(req_double.body[:eventdata])
      expect(payload).not_to have_key("description")
    end

    it "omits location when it is nil" do
      client.create_event(calendar, { title: "Meeting", location: nil,
                                      start_at: Time.utc(2026, 6, 1, 10) })
      payload = JSON.parse(req_double.body[:eventdata])
      expect(payload).not_to have_key("location")
    end

    it "includes description when it is a non-nil string" do
      client.create_event(calendar, { title: "Meeting", description: "Agenda here",
                                      start_at: Time.utc(2026, 6, 1, 10) })
      payload = JSON.parse(req_double.body[:eventdata])
      expect(payload["description"]).to eq("Agenda here")
    end

    it "includes location when it is an empty string" do
      client.create_event(calendar, { title: "Meeting", location: "",
                                      start_at: Time.utc(2026, 6, 1, 10) })
      payload = JSON.parse(req_double.body[:eventdata])
      expect(payload["location"]).to eq("")
    end
  end

  # ---------------------------------------------------------------------------
  # Fix 4 - update_event sends etag as a request header, not in the body
  # ---------------------------------------------------------------------------

  describe "#update_event" do
    let(:req_double) { fake_req }

    before do
      allow(conn).to receive(:put)
                       .with("#{base_url}/calendars/cal_001/events/evt_001")
                       .and_yield(req_double)
                       .and_return(ok_response("events" => [ { "uid" => "evt_001", "title" => "Updated", "dateandtime" => {} } ]))
    end

    it "sends the etag as a request header" do
      client.update_event(calendar, "evt_001", { title: "Updated" }, etag: "abc123")
      expect(req_double.headers["etag"]).to eq("abc123")
    end

    it "does not include etag in the body" do
      client.update_event(calendar, "evt_001", { title: "Updated" }, etag: "abc123")
      body = JSON.parse(req_double.body[:eventdata]) rescue req_double.body
      # body is the eventdata hash - etag must not appear there
      expect(req_double.body.keys).not_to include(:etag)
    end

    it "sends no etag header when etag is nil" do
      client.update_event(calendar, "evt_001", { title: "Updated" }, etag: nil)
      expect(req_double.headers).not_to have_key("etag")
    end

    it "raises ConflictError on 412" do
      allow(conn).to receive(:put)
                       .with("#{base_url}/calendars/cal_001/events/evt_001")
                       .and_yield(req_double)
                       .and_return(error_response(412))

      expect {
        client.update_event(calendar, "evt_001", { title: "Updated" }, etag: "stale")
      }.to raise_error(Calendars::ConflictError)
    end
  end

  # ---------------------------------------------------------------------------
  # Fix 4 - delete_event sends etag as a request header
  # ---------------------------------------------------------------------------

  describe "#delete_event" do
    let(:req_double) { fake_req }

    context "when an etag is provided by the caller" do
      before do
        allow(conn).to receive(:delete)
                         .with("#{base_url}/calendars/cal_001/events/evt_001")
                         .and_yield(req_double)
                         .and_return(ok_response({}))
      end

      it "sends the etag as a request header" do
        client.delete_event(calendar, "evt_001", etag: "xyz789")
        expect(req_double.headers["etag"]).to eq("xyz789")
      end

      it "does not put the etag in URL params" do
        client.delete_event(calendar, "evt_001", etag: "xyz789")
        expect(req_double.params).not_to have_key("etag")
      end
    end

    context "when no etag is provided" do
      let(:etag_response) do
        ok_response("events" => [ { "uid" => "evt_001", "etag" => "fetched_etag", "dateandtime" => {} } ])
      end

      before do
        # First call: GET to fetch the etag; second call: DELETE
        allow(conn).to receive(:get)
                         .with("#{base_url}/calendars/cal_001/events/evt_001")
                         .and_return(etag_response)
        allow(conn).to receive(:delete)
                         .with("#{base_url}/calendars/cal_001/events/evt_001")
                         .and_yield(req_double)
                         .and_return(ok_response({}))
      end

      it "fetches the current etag and sends it as a header" do
        client.delete_event(calendar, "evt_001")
        expect(conn).to have_received(:get)
                          .with("#{base_url}/calendars/cal_001/events/evt_001")
                          .once
        expect(req_double.headers["etag"]).to eq("fetched_etag")
      end
    end

    context "when the etag fetch fails" do
      before do
        allow(conn).to receive(:get)
                         .with("#{base_url}/calendars/cal_001/events/evt_001")
                         .and_return(error_response(500))
        allow(conn).to receive(:delete)
                         .with("#{base_url}/calendars/cal_001/events/evt_001")
                         .and_yield(req_double)
                         .and_return(ok_response({}))
      end

      it "proceeds with the delete without an etag header" do
        expect { client.delete_event(calendar, "evt_001") }.not_to raise_error
        expect(req_double.headers).not_to have_key("etag")
      end
    end
  end

  # ---------------------------------------------------------------------------
  # list_events_incremental clamps to a single slice
  #
  # The incremental pull runs every minute. If it inherited the persisted
  # multi-month window it would chunk into ~16 requests per calendar per minute,
  # so it must clamp to one <=30-day window around now (the 15-minute full sweep
  # covers the rest of the window).
  # ---------------------------------------------------------------------------

  describe "#list_events_incremental" do
    it "issues exactly one request even when the persisted window spans 455 days" do
      calendar.sync_window_start = 90.days.ago
      calendar.sync_window_end   = 365.days.from_now

      allow(conn).to receive(:get)
                       .with("#{base_url}/calendars/cal_001/events", anything)
                       .and_return(ok_response("events" => []))

      client.list_events_incremental(calendar)

      expect(conn).to have_received(:get).exactly(1).times
    end

    it "polls a window that starts no earlier than ~7 days ago and spans at most 30 days" do
      calendar.sync_window_start = 90.days.ago
      calendar.sync_window_end   = 365.days.from_now

      captured_range = nil
      allow(conn).to receive(:get)
                       .with("#{base_url}/calendars/cal_001/events", anything) do |_url, params|
        captured_range = JSON.parse(params[:range])
        ok_response("events" => [])
      end

      client.list_events_incremental(calendar)

      range_start = Time.parse(captured_range["start"])
      range_end   = Time.parse(captured_range["end"])
      expect(range_start).to be_within(1.minute).of(7.days.ago)
      expect(range_end - range_start).to be <= 30.days.to_i
    end
  end
end

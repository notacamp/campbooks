require "rails_helper"

RSpec.describe Calendars::EventWriter do
  let(:workspace) { create(:workspace) }
  let(:account) { create(:calendar_account, workspace: workspace) }
  let(:calendar) { create(:calendar, calendar_account: account) }
  let(:client) { instance_double(Google::CalendarClient) }

  # A temp id is assigned by Tools::CreateCalendarEvent before the job runs.
  let(:event) do
    create(:calendar_event,
      calendar: calendar,
      provider_event_id: "local-#{SecureRandom.uuid}",
      title: "Sprint Review",
      start_at: 2.days.from_now.beginning_of_hour,
      end_at: 2.days.from_now.beginning_of_hour + 1.hour,
      outbound_pending: true,
      status: :confirmed)
  end

  before do
    allow(account).to receive(:calendar_client).and_return(client)
  end

  describe "#call(:create)" do
    let(:remote_response) do
      {
        provider_event_id: "real_evt_abc",
        provider_etag: '"etag_001"',
        provider_sequence: 1,
        html_link: "https://calendar.google.com/event/real",
        conference_url: nil
      }
    end

    before { allow(client).to receive(:create_event).and_return(remote_response) }

    it "swaps the temp provider_event_id for the real one returned by the provider" do
      described_class.new(event).call(:create)
      expect(event.reload.provider_event_id).to eq("real_evt_abc")
    end

    it "clears outbound_pending after a successful create" do
      described_class.new(event).call(:create)
      expect(event.reload.outbound_pending).to be(false)
    end

    it "stores the provider etag for future conflict detection" do
      described_class.new(event).call(:create)
      expect(event.reload.provider_etag).to eq('"etag_001"')
    end

    it "sends no color to the provider (color belongs to the calendar, not the event)" do
      described_class.new(event).call(:create)
      expect(client).to have_received(:create_event) do |_calendar, attrs|
        expect(attrs).not_to have_key(:color)
      end
    end
  end

  describe "#call(:delete)" do
    before do
      allow(client).to receive(:delete_event).and_return(true)
    end

    it "marks the event as cancelled" do
      described_class.new(event).call(:delete)
      expect(event.reload.status).to eq("cancelled")
    end

    it "clears outbound_pending" do
      described_class.new(event).call(:delete)
      expect(event.reload.outbound_pending).to be(false)
    end
  end

  describe "#call(:update) with ConflictError" do
    let(:stale_etag) { '"etag_old"' }
    let(:fresh_etag) { '"etag_fresh"' }
    let(:remote_after_retry) do
      {
        provider_event_id: event.provider_event_id,
        provider_etag: '"etag_after_update"',
        provider_sequence: 3,
        html_link: event.html_link,
        conference_url: nil
      }
    end
    let(:fresh_event_attrs) do
      {
        provider_event_id: event.provider_event_id,
        provider_etag: fresh_etag,
        title: event.title,
        start_at: event.start_at,
        end_at: event.end_at
      }
    end

    before do
      event.update_columns(provider_etag: stale_etag)

      call_count = 0
      allow(client).to receive(:update_event) do
        call_count += 1
        if call_count == 1
          raise Calendars::ConflictError, "412 etag mismatch"
        else
          remote_after_retry
        end
      end
      allow(client).to receive(:get_event).and_return(fresh_event_attrs)
    end

    it "re-fetches the event to adopt the fresh etag" do
      described_class.new(event).call(:update)
      expect(client).to have_received(:get_event).once
    end

    it "retries the update with no etag guard (last-write-wins) and clears outbound_pending" do
      described_class.new(event).call(:update)
      expect(client).to have_received(:update_event).twice
      expect(event.reload.outbound_pending).to be(false)
    end

    it "adopts the fresh etag from the re-fetch before retrying" do
      described_class.new(event).call(:update)
      # Second update_event call received nil etag (no guard), so the row
      # ends up with the etag from the final remote_after_retry response.
      expect(event.reload.provider_etag).to eq('"etag_after_update"')
    end
  end
end

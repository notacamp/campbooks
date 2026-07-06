require "rails_helper"

RSpec.describe CalendarScanJob, type: :job do
  # Minimal event hash matching normalize_event output from Google::CalendarClient.
  def event_attrs(overrides = {})
    {
      provider_event_id: "evt_#{SecureRandom.hex(4)}",
      title: "Team standup",
      description: nil,
      location: nil,
      html_link: "https://calendar.google.com/event/1",
      conference_url: nil,
      start_at: 1.day.from_now.beginning_of_hour,
      end_at: 1.day.from_now.beginning_of_hour + 1.hour,
      start_time_zone: "UTC",
      end_time_zone: "UTC",
      all_day: false,
      status: "confirmed",
      rsvp_status: nil,
      is_organizer: true,
      attendees: [],
      provider_etag: '"etag_abc"',
      provider_sequence: 0,
      rrule: nil,
      recurring_event_provider_id: nil,
      original_start_at: nil
    }.merge(overrides)
  end

  let(:workspace) { create(:workspace) }
  let(:account) { create(:calendar_account, workspace: workspace) }
  let(:client) { instance_double(Google::CalendarClient) }

  before do
    # Stub calendar_client at the class level so fresh AR objects loaded inside
    # the job also get the fake client — same pattern as EmailScanJob spec.
    allow_any_instance_of(CalendarAccount).to receive(:calendar_client).and_return(client)
  end

  describe "#perform — full scan" do
    let!(:calendar) do
      create(:calendar, :primary, calendar_account: account, syncing: true, sync_token: nil)
    end

    before do
      allow(client).to receive(:calendar_list).and_return([
        {
          provider_calendar_id: calendar.provider_calendar_id,
          name: calendar.name,
          description: nil,
          color: "#3b82f6",
          time_zone: "UTC",
          is_primary: true,
          is_writable: true
        }
      ])
      allow(client).to receive(:list_events_full).and_return(
        { events: [ event_attrs ], next_sync_token: "tok_abc" }
      )
    end

    it "upserts events and stamps the sync token" do
      expect {
        described_class.perform_now(account.id, "full")
      }.to change(CalendarEvent, :count).by(1)
        .and change(CalendarSyncLog, :count).by(1)

      expect(calendar.reload.sync_token).to eq("tok_abc")
    end

    it "keeps a user-edited calendar color across full syncs" do
      calendar.update!(color: "#123456")
      described_class.perform_now(account.id, "full") # provider list says #3b82f6
      expect(calendar.reload.color).to eq("#123456")
    end

    it "auto-enables the primary calendar when first discovered" do
      account.calendars.destroy_all

      allow(client).to receive(:calendar_list).and_return([
        {
          provider_calendar_id: "primary@gmail.com",
          name: "Primary Calendar",
          description: nil,
          color: "#3b82f6",
          time_zone: "UTC",
          is_primary: true,
          is_writable: true
        }
      ])
      allow(client).to receive(:list_events_full).and_return(
        { events: [], next_sync_token: "tok_x" }
      )

      described_class.perform_now(account.id, "full")
      primary = account.reload.calendars.find_by(is_primary: true)
      expect(primary).to be_present
      expect(primary.syncing).to be(true)
      expect(primary.color).to eq("#3b82f6") # provider color seeds first discovery only
    end

    it "records a completed sync log" do
      described_class.perform_now(account.id, "full")
      log = CalendarSyncLog.last
      expect(log.status).to eq("completed")
    end

    it "releases the scan slot on success" do
      described_class.perform_now(account.id, "full")
      expect(account.reload.scanning).to be(false)
    end
  end

  describe "#perform — incremental scan" do
    let!(:calendar) do
      create(:calendar, calendar_account: account, syncing: true, sync_token: "old_tok")
    end

    it "uses the incremental path when a sync token is present" do
      allow(client).to receive(:list_events_incremental).and_return(
        { events: [ event_attrs ], next_sync_token: "new_tok" }
      )

      described_class.perform_now(account.id, "incremental")

      expect(client).to have_received(:list_events_incremental)
      expect(calendar.reload.sync_token).to eq("new_tok")
    end

    it "falls back to full resync when the sync token is expired (HTTP 410)" do
      allow(client).to receive(:list_events_incremental)
        .and_raise(Calendars::SyncTokenExpired, "token expired")

      expect {
        described_class.perform_now(account.id, "incremental")
      }.to have_enqueued_job(Calendars::FullResyncJob)
    end
  end

  describe "loop-avoidance — etag deduplication" do
    let!(:calendar) do
      create(:calendar, calendar_account: account, syncing: true, sync_token: "tok")
    end
    let(:existing_etag) { '"etag_abc123"' }
    let!(:existing_event) do
      create(:calendar_event,
        calendar: calendar,
        provider_event_id: "evt_existing",
        provider_etag: existing_etag,
        title: "Original Title",
        start_at: 1.day.from_now,
        end_at: 1.day.from_now + 1.hour)
    end

    it "does not update a row whose etag is unchanged" do
      same_etag_event = event_attrs(
        provider_event_id: "evt_existing",
        provider_etag: existing_etag,
        title: "Changed Title"
      )
      allow(client).to receive(:list_events_incremental).and_return(
        { events: [ same_etag_event ], next_sync_token: "tok2" }
      )

      described_class.perform_now(account.id, "incremental")
      expect(existing_event.reload.title).to eq("Original Title")
    end
  end

  describe "tombstoning cancelled events" do
    let!(:calendar) do
      create(:calendar, calendar_account: account, syncing: true, sync_token: "tok")
    end
    let!(:event) do
      create(:calendar_event,
        calendar: calendar,
        provider_event_id: "evt_will_cancel",
        status: :confirmed,
        start_at: 1.day.from_now,
        end_at: 1.day.from_now + 1.hour)
    end

    before do
      cancelled = event_attrs(
        provider_event_id: event.provider_event_id,
        status: "cancelled",
        start_at: nil,
        end_at: nil
      )
      allow(client).to receive(:list_events_incremental).and_return(
        { events: [ cancelled ], next_sync_token: "tok2" }
      )
    end

    it "marks the event as cancelled without deleting the row" do
      described_class.perform_now(account.id, "incremental")
      expect(event.reload.status).to eq("cancelled")
      expect(CalendarEvent.exists?(event.id)).to be(true)
    end

    it "excludes the cancelled event from .visible" do
      described_class.perform_now(account.id, "incremental")
      expect(CalendarEvent.visible).not_to include(event.reload)
    end
  end

  describe "stale scan reconciliation" do
    let!(:calendar) do
      create(:calendar, calendar_account: account, syncing: true, sync_token: "tok")
    end

    before do
      allow(client).to receive(:list_events_incremental).and_return(
        { events: [], next_sync_token: "tok2" }
      )
    end

    it "clears a stuck scanning flag from a dead worker" do
      account.update_columns(scanning: true, scan_started_at: (CalendarAccount::SCAN_STALE_AFTER + 1.minute).ago)
      allow_any_instance_of(described_class).to receive(:claim_scan_slot).and_return(false)

      described_class.perform_now(account.id)
      expect(account.reload.scanning).to be(false)
    end

    it "leaves a fresh scanning flag alone" do
      account.update_columns(scanning: true, scan_started_at: 30.seconds.ago)
      allow_any_instance_of(described_class).to receive(:claim_scan_slot).and_return(false)

      described_class.perform_now(account.id)
      expect(account.reload.scanning).to be(true)
    end
  end

  describe "error handling" do
    let!(:calendar) do
      create(:calendar, calendar_account: account, syncing: true, sync_token: "tok")
    end

    before do
      allow(client).to receive(:list_events_incremental)
        .and_raise(StandardError, "API outage")
    end

    it "marks the sync log as failed" do
      described_class.perform_now(account.id, "incremental")
      log = CalendarSyncLog.last
      expect(log.status).to eq("failed")
      expect(log.error_messages).to be_present
    end

    it "always releases the scan slot even on failure" do
      described_class.perform_now(account.id, "incremental")
      expect(account.reload.scanning).to be(false)
    end
  end

  # Converted from test/jobs/calendar_scan_job_test.rb — ServiceUnavailable
  # handling when a Google identity has no Calendar provisioned.
  describe "Calendars::ServiceUnavailable handling" do
    # The account starts with no calendars, so refresh_calendar_list is called
    # even on an incremental scope (account.calendars.empty? == true).

    context "when calendar_list raises Calendars::ServiceUnavailable" do
      before do
        allow(client).to receive(:calendar_list)
          .and_raise(Calendars::ServiceUnavailable, "Google account is not signed up for Google Calendar")
      end

      it "deactivates the account with calendar_service_unavailable and does not re-raise" do
        expect { described_class.perform_now(account.id, "incremental") }.not_to raise_error
        account.reload
        expect(account.active?).to be(false)
        expect(account.deactivation_reason).to eq("calendar_service_unavailable")
        expect(account.deactivated_for_service?).to be(true)
      end

      it "does not re-raise Calendars::ServiceUnavailable out of the job" do
        expect { described_class.perform_now(account.id, "incremental") }.not_to raise_error
      end
    end

    context "when calendar_list returns normally" do
      before do
        allow(client).to receive(:calendar_list).and_return([])
      end

      it "does not deactivate the account" do
        described_class.perform_now(account.id, "incremental")
        account.reload
        expect(account.active?).to be(true)
        expect(account.deactivation_reason).to be_nil
      end
    end

    context "when the account is already inactive" do
      before do
        account.deactivate_for!(:calendar_service_unavailable)
        allow(client).to receive(:calendar_list).and_return([])
      end

      it "is skipped by the active scope so calendar_list is never called" do
        described_class.perform_now(account.id, "incremental")
        expect(client).not_to have_received(:calendar_list)
        expect(account.reload.deactivation_reason).to eq("calendar_service_unavailable")
      end
    end
  end
end

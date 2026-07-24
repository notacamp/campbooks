require "rails_helper"

# Guest invites on calendar events: the attendee_emails form field replaces the
# guest list (preserving stored responses) — but only for events the user
# organizes. The provider write (invitation emails ride on sendUpdates=all) is
# covered in event_writer_spec / the client specs.
RSpec.describe "Calendar event guests", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:calendar_account, workspace: workspace) }
  let(:calendar) { create(:calendar, calendar_account: account, is_writable: true, syncing: true) }

  before do
    create(:calendar_account_user, :editor, user: user, calendar_account: account)
    sign_in(user)
  end

  describe "POST /calendar_events" do
    let(:params) do
      { calendar_event: {
        calendar_id: calendar.id,
        title: "Kickoff",
        start_at: 2.days.from_now.change(hour: 14).iso8601,
        end_at: 2.days.from_now.change(hour: 15).iso8601,
        attendee_emails: "maya@example.com, rui@example.com"
      } }
    end

    it "stores the guests as pending and marks the event organizer-owned" do
      expect { post calendar_events_path, params: params }.to change(CalendarEvent, :count).by(1)
      event = CalendarEvent.order(:created_at).last
      expect(event.is_organizer).to be(true)
      expect(event.attendees).to eq([
        { "email" => "maya@example.com", "rsvp_status" => "needs_action" },
        { "email" => "rui@example.com", "rsvp_status" => "needs_action" }
      ])
      expect(Calendars::EventWriteJob).to have_been_enqueued.with(event.id, "create")
    end
  end

  describe "PATCH /calendar_events/:id" do
    let(:stored) do
      [
        { "email" => "maya@example.com", "name" => "Maya", "rsvp_status" => "accepted" },
        { "email" => "rui@example.com", "rsvp_status" => "needsAction" }
      ]
    end

    def patch_event(event, emails)
      patch calendar_event_path(event), params: { calendar_event: { title: event.title, attendee_emails: emails } }
    end

    context "when the user organizes the event" do
      let(:event) { create(:calendar_event, calendar: calendar, is_organizer: true, attendees: stored) }

      it "replaces the list, preserving responses of guests that stay" do
        patch_event(event, "maya@example.com, sam@example.com")
        expect(event.reload.attendees).to eq([
          { "email" => "maya@example.com", "name" => "Maya", "rsvp_status" => "accepted" },
          { "email" => "sam@example.com", "rsvp_status" => "needs_action" }
        ])
      end
    end

    context "when the user merely received the invite" do
      let(:event) { create(:calendar_event, calendar: calendar, is_organizer: false, attendees: stored) }

      it "ignores attendee_emails entirely" do
        patch_event(event, "hijack@example.com")
        expect(event.reload.attendees).to eq(stored)
        expect(response).to have_http_status(:redirect).or have_http_status(:ok)
      end
    end
  end
end

require "rails_helper"

# Covers PATCH /calendar_events/:id/reschedule — the endpoint behind drag-to-
# reschedule on the Day/Week time grids. The drag gesture itself lives in the
# calendar_dnd Stimulus controller (JS); this guards the server contract it relies
# on: writable events move + sync, everything else is refused without mutating.
RSpec.describe "Calendar event reschedule", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:calendar_account, workspace: workspace) }
  let(:calendar) { create(:calendar, calendar_account: account) }
  let(:event) do
    create(:calendar_event, calendar: calendar,
                            start_at: Time.zone.parse("2026-06-22 09:00"),
                            end_at: Time.zone.parse("2026-06-22 10:00"))
  end

  let(:new_start) { "2026-06-22T11:00" }
  let(:new_end) { "2026-06-22T12:00" }

  before { sign_in(user) }

  def reschedule!(target = event, start_at: new_start, end_at: new_end)
    patch reschedule_calendar_event_path(target), params: { start_at:, end_at: }
  end

  context "when the user can write the calendar (editor)" do
    before { create(:calendar_account_user, :editor, user: user, calendar_account: account) }

    it "moves the event and flags it for outbound sync" do
      expect { reschedule! }.to change { event.reload.start_at }.to(Time.zone.parse(new_start))

      expect(response).to have_http_status(:ok)
      expect(event.reload.end_at).to eq(Time.zone.parse(new_end))
      expect(event.reload.outbound_pending).to be(true)
    end

    it "enqueues EventWriteJob as a single-occurrence update" do
      expect { reschedule! }
        .to have_enqueued_job(Calendars::EventWriteJob).with(event.id, "update", "this")
    end
  end

  context "when the user only has read access (viewer)" do
    before { create(:calendar_account_user, :viewer, user: user, calendar_account: account) }

    it "is forbidden and leaves the event untouched" do
      expect { reschedule! }.not_to(change { event.reload.start_at })

      expect(response).to have_http_status(:forbidden)
      expect(event.reload.outbound_pending).to be(false)
    end

    it "does not enqueue the writer" do
      expect { reschedule! }.not_to have_enqueued_job(Calendars::EventWriteJob)
    end
  end

  context "when the provider marks the calendar non-writable" do
    let(:calendar) { create(:calendar, :read_only, calendar_account: account) }

    before { create(:calendar_account_user, :editor, user: user, calendar_account: account) }

    it "is forbidden even though the user can write the account" do
      expect { reschedule! }.not_to(change { event.reload.start_at })
      expect(response).to have_http_status(:forbidden)
    end
  end

  context "when the event is not accessible to the user" do
    let(:other_workspace) { create(:workspace) }
    let(:other_account) { create(:calendar_account, workspace: other_workspace) }
    let(:other_calendar) { create(:calendar, calendar_account: other_account) }
    let(:foreign_event) { create(:calendar_event, calendar: other_calendar) }

    it "returns 404 rather than leaking the event's existence" do
      reschedule!(foreign_event)
      expect(response).to have_http_status(:not_found)
    end
  end
end

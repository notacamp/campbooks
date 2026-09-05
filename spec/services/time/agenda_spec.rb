require "rails_helper"

RSpec.describe Time::Agenda do
  include Rails.application.routes.url_helpers

  let(:user) { create(:user) }
  let(:workspace) { user.workspace }
  let(:window) { { from: Time.current.beginning_of_day, to: 7.days.from_now.end_of_day } }

  describe "deadlines Scout found in a document" do
    let!(:document) do
      create(:document, workspace: workspace).tap do |doc|
        doc.assign_title("Seguro Renovação 2026")
        doc.save!
      end
    end
    let!(:reminder) do
      create(:reminder, workspace: workspace, source: document, reminder_type: :renewal,
                        title: "Policy renewal", due_at: 2.days.from_now, all_day: true)
    end

    it "renders the deadline with the document as its source (regression: Document has no #title)" do
      items = described_class.for(user, **window)
      item = items.find(&:deadline?)

      expect(item).to be_present
      expect(item.title).to eq("Policy renewal")
      expect(item.source_label).to include("Seguro Renovação 2026")
      expect(item.source_path).to eq(document_path(document))
    end
  end

  describe "event emphasis enrichment" do
    let(:calendar_account) { create(:calendar_account, workspace: workspace) }
    let(:calendar)         { create(:calendar, calendar_account: calendar_account, syncing: true, color: "#4a90e2") }

    before do
      create(:calendar_account_user, calendar_account: calendar_account, user: user, can_read: true)
    end

    it "assigns :normal emphasis to a plain event with no notable attendees" do
      create(:calendar_event, calendar: calendar,
             start_at: 2.hours.from_now, end_at: 3.hours.from_now, rsvp_status: :accepted)
      items = described_class.for(user, **window)
      event = items.find(&:event?)
      expect(event).to be_present
      expect(event.emphasis).to eq(:normal)
    end

    it "assigns :quiet when the user declined the event" do
      create(:calendar_event, calendar: calendar,
             start_at: 2.hours.from_now, end_at: 3.hours.from_now, rsvp_status: :declined)
      items = described_class.for(user, **window)
      event = items.find(&:event?)
      expect(event).to be_present
      expect(event).to be_quiet
    end
  end
end

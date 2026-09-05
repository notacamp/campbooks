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

    def weighted_person(name:, email:, weight:, reasons: [ { "key" => "replies_fast", "params" => { "hours" => 3 } } ])
      person  = create(:person, workspace: workspace, name: name)
      contact = create(:contact, workspace: workspace, person: person, email: email)
      AttentionWeight.create!(user: user, workspace: workspace, subject: person, weight: weight, confidence: 0.9,
                              reasons: reasons, computed_at: Time.current)
      [ person, contact ]
    end

    def meeting_with(contact)
      create(:calendar_event, calendar: calendar, start_at: 2.hours.from_now, end_at: 3.hours.from_now,
             rsvp_status: :accepted, attendees: [ { "email" => contact.email, "rsvp_status" => "accepted" } ])
    end

    it "marks a meeting with someone who matters :prep, with the why line, first name and detail" do
      _person, contact = weighted_person(name: "Sofia Martins", email: "sofia@brightloop.example", weight: 0.9)
      meeting_with(contact)

      event = described_class.for(user, **window).find(&:event?)

      expect(event).to be_prep
      expect(event.why).to eq("with Sofia, you usually answer within 3 hours")
      expect(event.prep_name).to eq("Sofia")
      expect(event.prep_detail).to eq("you usually answer within 3 hours")
    end

    it "quotes the open item with that person as the prep detail" do
      person, contact = weighted_person(name: "Sofia Martins", email: "sofia@brightloop.example", weight: 0.9)
      PeopleStanding.create!(user: user, workspace: workspace, counterpart: person, name: "Sofia Martins",
                             needs_you: true, standing_kind: "attention", verb: "reply", subject: "Q3 deck",
                             wait_days: 2, refreshed_at: Time.current)
      meeting_with(contact)

      event = described_class.for(user, **window).find(&:event?)

      expect(event.why).to include("open: Q3 deck, asked 2 days ago")
      expect(event.prep_detail).to eq("Q3 deck is still open, asked 2 days ago")
    end

    it "leaves a meeting with someone below the prep threshold :normal" do
      _person, contact = weighted_person(name: "Rui Santos", email: "rui@cloudhost.example", weight: 0.4)
      meeting_with(contact)

      event = described_class.for(user, **window).find(&:event?)
      expect(event.emphasis).to eq(:normal)
      expect(event.why).to be_nil
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

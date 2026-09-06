require "rails_helper"

RSpec.describe Time::Agenda do
  include Rails.application.routes.url_helpers

  # Pin the clock to a fixed midday so day-bucketing / overdue-pinning never
  # straddles midnight mid-run.
  around { |ex| travel_to(Time.utc(2026, 9, 7, 12, 0, 0)) { ex.run } }

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

  describe "asks" do
    before { allow(Features).to receive(:tasks?).and_return(true) }

    def ask(**attrs)
      workspace.tasks.create!({ title: "Ask #{SecureRandom.hex(2)}", status: :todo, priority: :normal }.merge(attrs))
    end

    it "renders a suggested dated ask as a row labelled Scout suggested" do
      task = ask(status: :suggested, ai_suggested: true, due_at: 2.days.from_now)
      item = described_class.for(user, **window).find { |i| i.task? && i.record == task }

      expect(item).to be_present
      expect(item.source_label).to include(I18n.t("time.agenda.source.scout_suggested"))
      expect(item.actions).to include(:done, :change_date, :snooze, :dismiss_ask)
    end

    it "excludes snoozed asks" do
      snoozed = ask(due_at: 2.days.from_now, snoozed_until: 1.week.from_now)
      records = described_class.for(user, **window).map(&:record)
      expect(records).not_to include(snoozed)
    end

    it "pins an overdue ask into today" do
      overdue = ask(due_at: 2.days.ago)
      item = described_class.for(user, **window).find { |i| i.task? && i.record == overdue }

      expect(item).to be_present
      expect(item.overdue).to be(true)
      expect(item.day).to eq(Time.current.in_time_zone(user.effective_time_zone).to_date)
    end

    it "appends the held slot to the source label when Scout holds time" do
      task = ask(due_at: 3.days.from_now)
      FocusBlock.create!(workspace: workspace, user: user, task: task, title: "Focus",
                         start_at: 1.day.from_now.change(hour: 10),
                         end_at: 1.day.from_now.change(hour: 10) + 45.minutes, status: :proposed)
      item = described_class.for(user, **window).find { |i| i.task? && i.record == task }

      expect(item.source_label).to include("held")
    end

    it "a focus block for an ask carries done_ask" do
      task = ask
      block = FocusBlock.create!(workspace: workspace, user: user, task: task, title: "Focus",
                                 start_at: 1.day.from_now.change(hour: 10),
                                 end_at: 1.day.from_now.change(hour: 10) + 45.minutes, status: :proposed)
      item = described_class.for(user, **window).find { |i| i.focus? && i.record == block }

      expect(item.actions).to include(:done_ask)
    end
  end

  describe "#undated" do
    before { allow(Features).to receive(:tasks?).and_return(true) }

    it "lists live undated asks (never in #items), excludes held ones, and carries the three ways out" do
      undated = workspace.tasks.create!(title: "No date", status: :todo)
      held = workspace.tasks.create!(title: "Held undated", status: :todo)
      FocusBlock.create!(workspace: workspace, user: user, task: held, title: "Focus",
                         start_at: 1.day.from_now, end_at: 1.day.from_now + 45.minutes, status: :proposed)

      agenda = described_class.new(user, **window)
      undated_items = agenda.undated

      expect(undated_items.map(&:record)).to include(undated)
      expect(undated_items.map(&:record)).not_to include(held)
      expect(undated_items).to all(be_undated)
      expect(undated_items.first.actions).to include(:schedule, :hold, :done)
      expect(agenda.items.map(&:record)).not_to include(undated)
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

  describe "hand-off" do
    before { allow(Features).to receive(:tasks?).and_return(true) }

    let(:teammate) do
      workspace.users.create!(name: "Ana Lima", email_address: "ana-#{SecureRandom.hex(3)}@example.com", password: "password123")
    end

    def ask(**attrs)
      workspace.tasks.create!({ title: "Ask #{SecureRandom.hex(2)}", status: :todo, priority: :normal }.merge(attrs))
    end

    it "shows the assigner a handed dated ask with handed: true and take_back/done" do
      task = ask(status: :todo, due_at: 2.days.from_now)
      Asks::HandOff.call(task, to: teammate, by: user)

      item = described_class.for(user, **window).find { |i| i.task? && i.record == task }
      expect(item).to be_present
      expect(item.handed?).to be(true)
      expect(item.actions).to eq(%i[take_back done])
      expect(item.record.handed_to).to eq(teammate)
    end

    it "shows the assignee the ask with handed_by provenance" do
      task = ask(status: :todo, due_at: 2.days.from_now)
      Asks::HandOff.call(task, to: teammate, by: user)

      item = described_class.for(teammate, **window).find { |i| i.task? && i.record == task }
      expect(item).to be_present
      expect(item.handed?).to be(false)
      expect(item.source_label).to include("from #{user.name.split.first}")
    end
  end
end

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
end

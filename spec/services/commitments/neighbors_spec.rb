require "rails_helper"

RSpec.describe Commitments::Neighbors do
  # Pin the clock so date arithmetic in queries is stable.
  around { |ex| travel_to(Time.zone.parse("2026-07-01 10:00:00")) { ex.run } }

  let(:workspace) { create(:workspace) }
  let(:due_at)    { Time.zone.parse("2026-07-15 09:00:00") }

  let(:account)  { create(:calendar_account, workspace: workspace) }
  let(:calendar) { create(:calendar, calendar_account: account) }

  let(:other_workspace) { create(:workspace) }
  let(:other_account)   { create(:calendar_account, workspace: other_workspace) }
  let(:other_calendar)  { create(:calendar, calendar_account: other_account) }

  def neighbors(overrides = {})
    described_class.around(workspace: workspace, due_at: due_at, **overrides)
  end

  describe "return [] when due_at is nil" do
    it "returns an empty array" do
      expect(described_class.around(workspace: workspace, due_at: nil)).to eq([])
    end
  end

  describe "reminder neighbors" do
    it "includes reminders within the window" do
      r = create(:reminder, workspace: workspace, due_at: due_at)
      expect(neighbors.map(&:record)).to include(r)
      expect(neighbors.map(&:kind)).to all(be_a(String))
    end

    it "excludes reminders outside the window" do
      far = create(:reminder, workspace: workspace, due_at: due_at + 3.days)
      expect(neighbors.map(&:record)).not_to include(far)
    end

    it "includes dismissed reminders" do
      dismissed = create(:reminder, workspace: workspace, due_at: due_at, status: :confirmed)
      dismissed.update!(status: :dismissed)
      expect(neighbors.map(&:record)).to include(dismissed)
    end

    it "excludes reminders from another workspace" do
      other_src = create(:document, workspace: other_workspace)
      other = create(:reminder, workspace: other_workspace, source: other_src, due_at: due_at)
      expect(neighbors.map(&:record)).not_to include(other)
    end
  end

  describe "task neighbors" do
    # No :task factory — use the model directly (mirrors tasks/builder_spec.rb).
    def make_task(ws, attrs = {})
      Task.create!({ workspace: ws, title: "Test task", status: :todo, priority: :normal, confidence: 0.9 }.merge(attrs))
    end

    it "includes tasks within the window that have a due_at" do
      t = make_task(workspace, due_at: due_at)
      expect(neighbors.map(&:record)).to include(t)
    end

    it "includes done tasks within the window" do
      done = make_task(workspace, due_at: due_at, status: :done)
      expect(neighbors.map(&:record)).to include(done)
    end

    it "excludes tasks without a due_at" do
      dateless = make_task(workspace, due_at: nil)
      expect(neighbors.map(&:record)).not_to include(dateless)
    end

    it "excludes tasks from another workspace" do
      other_t = make_task(other_workspace, due_at: due_at)
      expect(neighbors.map(&:record)).not_to include(other_t)
    end
  end

  describe "calendar event neighbors" do
    it "includes non-cancelled workspace events within the window" do
      ev = create(:calendar_event, calendar: calendar, start_at: due_at, end_at: due_at + 1.hour)
      expect(neighbors.map(&:record)).to include(ev)
    end

    it "excludes cancelled events" do
      cancelled = create(:calendar_event, :cancelled, calendar: calendar, start_at: due_at, end_at: due_at + 1.hour)
      expect(neighbors.map(&:record)).not_to include(cancelled)
    end

    it "excludes events from another workspace" do
      other_ev = create(:calendar_event, calendar: other_calendar, start_at: due_at, end_at: due_at + 1.hour)
      expect(neighbors.map(&:record)).not_to include(other_ev)
    end
  end

  describe "sorting by temporal distance" do
    it "returns neighbors sorted by absolute distance from due_at" do
      far_reminder  = create(:reminder, workspace: workspace, due_at: due_at - 20.hours)
      near_reminder = create(:reminder, workspace: workspace, due_at: due_at - 2.hours)

      records = neighbors.select { |n| n.kind == "reminder" }.map(&:record)
      near_idx = records.index(near_reminder)
      far_idx  = records.index(far_reminder)
      expect(near_idx).to be < far_idx
    end
  end

  describe "kind labels" do
    it "labels reminders, tasks, and calendar events correctly" do
      create(:reminder, workspace: workspace, due_at: due_at)
      Task.create!(workspace: workspace, due_at: due_at, title: "Kind label task", status: :todo, priority: :normal, confidence: 0.9)
      create(:calendar_event, calendar: calendar, start_at: due_at, end_at: due_at + 1.hour)

      kinds = neighbors.map(&:kind)
      expect(kinds).to include("reminder", "task", "calendar_event")
    end
  end
end

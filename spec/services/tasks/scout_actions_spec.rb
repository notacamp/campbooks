require "rails_helper"

RSpec.describe Tasks::ScoutActions do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:task) do
    Task.create!(workspace: workspace, title: "Finish the van registration",
                 status: :todo, created_by: user)
  end

  describe ".auto_safe?" do
    it "allows only the task tools" do
      expect(described_class.auto_safe?("set_due_date")).to be true
      expect(described_class.auto_safe?("set_reminder")).to be true
      expect(described_class.auto_safe?("archive")).to be false
      expect(described_class.auto_safe?("forward_email")).to be false
    end
  end

  describe "set_due_date" do
    it "sets an all-day due date from a date-only value" do
      result = described_class.run("set_due_date", task: task, args: { "due_at" => "2026-08-01" })

      expect(result[:success]).to be true
      expect(task.reload.due_at.to_date).to eq(Date.new(2026, 8, 1))
      expect(task.all_day).to be true
    end

    it "sets a timed due date from a full datetime" do
      described_class.run("set_due_date", task: task, args: { "due_at" => "2026-08-03T09:30:00Z" })

      expect(task.reload.due_at).to eq(Time.zone.parse("2026-08-03T09:30:00Z"))
      expect(task.all_day).to be false
    end

    it "fails cleanly on an unparseable date" do
      result = described_class.run("set_due_date", task: task, args: { "due_at" => "not-a-date" })

      expect(result[:success]).to be false
      expect(task.reload.due_at).to be_nil
    end
  end

  describe "set_reminder" do
    it "sets the due date and creates the task's pending deadline reminder" do
      result = described_class.run("set_reminder", task: task, args: { "due_at" => "2026-08-01" })

      expect(result[:success]).to be true
      reminder = task.reminders.sole
      expect(reminder.reminder_type).to eq("deadline")
      expect(reminder.status).to eq("pending")
      expect(reminder.due_at.to_date).to eq(Date.new(2026, 8, 1))
      expect(reminder.title).to eq(task.title)
      expect(reminder.confidence).to eq(1.0)
      expect(task.reload.due_at.to_date).to eq(Date.new(2026, 8, 1))
    end

    it "re-running moves the existing reminder instead of duplicating it" do
      described_class.run("set_reminder", task: task, args: { "due_at" => "2026-08-01" })
      described_class.run("set_reminder", task: task, args: { "due_at" => "2026-09-15" })

      expect(task.reminders.count).to eq(1)
      expect(task.reminders.sole.due_at.to_date).to eq(Date.new(2026, 9, 15))
    end

    it "uses the task's existing due date when no date is given" do
      task.update!(due_at: Time.zone.parse("2026-08-10T12:00:00Z"))

      result = described_class.run("set_reminder", task: task, args: {})

      expect(result[:success]).to be true
      expect(task.reminders.sole.due_at).to eq(task.due_at)
    end

    it "fails when neither the args nor the task carry a due date" do
      result = described_class.run("set_reminder", task: task, args: {})

      expect(result[:success]).to be false
      expect(task.reminders).to be_empty
    end
  end

  it "returns a failure for an unknown tool" do
    result = described_class.run("archive", task: task, args: {})

    expect(result[:success]).to be false
  end
end

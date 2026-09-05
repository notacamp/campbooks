require "rails_helper"

RSpec.describe Task do
  before do
    @ws = Workspace.create!(name: "Tasks Test WS")
    @user = @ws.users.create!(name: "Tess", email_address: "tess-tasks@example.com", password: "password123")
  end

  it "requires a title" do
    task = @ws.tasks.build(status: :todo, priority: :normal)
    expect(task).not_to be_valid
    expect(task.errors[:title]).to include("can't be blank")
  end

  it "status defaults to suggested and priority to normal" do
    task = @ws.tasks.create!(title: "Triage me")
    expect(task.status).to eq("suggested")
    expect(task).to be_priority_normal
  end

  it "fingerprint_for is deterministic and normalizes the title" do
    a = Task.fingerprint_for(source_type: "EmailMessage", source_id: "abc", title: "Pay Invoice")
    b = Task.fingerprint_for(source_type: "EmailMessage", source_id: "abc", title: "  pay invoice  ")
    expect(b).to eq(a)
  end

  it "accessible_to is workspace-scoped and fails closed for a nil user" do
    mine   = @ws.tasks.create!(title: "Mine")
    theirs = Workspace.create!(name: "Other WS").tasks.create!(title: "Theirs")

    expect(Task.accessible_to(@user)).to include(mine)
    expect(Task.accessible_to(@user)).not_to include(theirs)
    expect(Task.accessible_to(nil)).to be_empty
  end

  it "move_to_status! to done stamps completed_at and logs events" do
    task = @ws.tasks.create!(title: "Ship it", status: :todo, created_by: @user)

    task.move_to_status!(:done, by: @user)

    expect(task).to be_done
    expect(task.completed_at).not_to be_nil
    names = Event.for_subject(task).pluck(:name)
    expect(names).to include("task.status_changed")
    expect(names).to include("task.completed")
  end

  it "move_to_status! away from done clears completed_at" do
    task = @ws.tasks.create!(title: "Reopen", status: :done, completed_at: Time.current, created_by: @user)

    task.move_to_status!(:in_progress, by: @user)

    expect(task).to be_in_progress
    expect(task.completed_at).to be_nil
  end

  it "move_to_status! rejects an unknown status" do
    task = @ws.tasks.create!(title: "Nope", status: :todo)
    expect { task.move_to_status!(:nonsense) }.to raise_error(ArgumentError)
  end

  it "overdue scope returns only active, past-due tasks" do
    overdue   = @ws.tasks.create!(title: "late", status: :todo, due_at: 1.day.ago)
    upcoming  = @ws.tasks.create!(title: "soon", status: :todo, due_at: 1.day.from_now)
    done_late = @ws.tasks.create!(title: "done late", status: :done, due_at: 1.day.ago)

    expect(Task.overdue).to include(overdue)
    expect(Task.overdue).not_to include(upcoming)
    expect(Task.overdue).not_to include(done_late)
  end

  it "shares the Tag model with emails through task_tags" do
    tag  = Tag.create!(workspace: @ws, name: "finance", color: "#6366f1")
    task = @ws.tasks.create!(title: "Budget")
    task.tags << tag

    expect(task.reload.tags).to include(tag)
    expect(tag.reload.tasks).to include(task)
  end

  describe "ask scopes" do
    it "awake excludes future-snoozed tasks; snoozed is the inverse" do
      awake_task = @ws.tasks.create!(title: "Awake", status: :todo)
      lapsed = @ws.tasks.create!(title: "Lapsed", status: :todo, snoozed_until: 1.hour.ago)
      snoozed = @ws.tasks.create!(title: "Snoozed", status: :todo, snoozed_until: 1.week.from_now)

      expect(Task.awake).to include(awake_task, lapsed)
      expect(Task.awake).not_to include(snoozed)
      expect(Task.snoozed).to contain_exactly(snoozed)
    end

    it "live is accepted-or-suggested, not archived, not snoozed, not done/cancelled" do
      suggested = @ws.tasks.create!(title: "Suggested", status: :suggested)
      todo = @ws.tasks.create!(title: "Todo", status: :todo)
      done = @ws.tasks.create!(title: "Done", status: :done)
      snoozed = @ws.tasks.create!(title: "Snoozed", status: :todo, snoozed_until: 1.week.from_now)
      archived = @ws.tasks.create!(title: "Archived", status: :todo)
      archived.archive!(by: @user)

      live = Task.live
      expect(live).to include(suggested, todo)
      expect(live).not_to include(done, snoozed, archived)
    end

    it "dated / undated split on due_at" do
      dated = @ws.tasks.create!(title: "Dated", status: :todo, due_at: 1.day.from_now)
      undated = @ws.tasks.create!(title: "Undated", status: :todo)

      expect(Task.dated).to include(dated)
      expect(Task.dated).not_to include(undated)
      expect(Task.undated).to include(undated)
      expect(Task.undated).not_to include(dated)
    end
  end

  describe "ask actions" do
    it "accept! promotes a suggestion to todo and clears any snooze" do
      task = @ws.tasks.create!(title: "Confirm", status: :suggested, snoozed_until: 1.week.from_now)
      task.accept!(by: @user)

      expect(task.reload).to be_todo
      expect(task.snoozed_until).to be_nil
    end

    it "accept! is a no-op (still true) for an already-accepted ask" do
      task = @ws.tasks.create!(title: "Open", status: :todo)
      expect(task.accept!(by: @user)).to be(true)
      expect(task.reload).to be_todo
    end

    it "snooze! sets snoozed_until, keeps status, and publishes task.snoozed" do
      task = @ws.tasks.create!(title: "Later", status: :suggested)
      task.snooze!(by: @user)

      expect(task.reload).to be_suggested
      expect(task.snoozed?).to be(true)
      expect(Event.for_subject(task).pluck(:name)).to include("task.snoozed")
    end

    it "schedule! sets due_at to 09:30 local, accepts, and publishes task.scheduled" do
      zone = ActiveSupport::TimeZone["Europe/Lisbon"]
      task = @ws.tasks.create!(title: "Ship", status: :suggested)

      task.schedule!(Date.new(2026, 9, 11), zone: zone, by: @user)
      local = task.reload.due_at.in_time_zone(zone)

      expect(local.hour).to eq(9)
      expect(local.min).to eq(30)
      expect(task.all_day).to be(false)
      expect(task).to be_todo
      expect(Event.for_subject(task).pluck(:name)).to include("task.scheduled")
    end

    it "held_block returns the earliest held focus block for the ask" do
      task = @ws.tasks.create!(title: "Deck", status: :todo)
      later = FocusBlock.create!(workspace: @ws, user: @user, task: task, title: "Focus: Deck",
                                 start_at: 2.days.from_now, end_at: 2.days.from_now + 45.minutes, status: :proposed)
      earlier = FocusBlock.create!(workspace: @ws, user: @user, task: task, title: "Focus: Deck",
                                   start_at: 1.day.from_now, end_at: 1.day.from_now + 45.minutes, status: :proposed)
      FocusBlock.create!(workspace: @ws, user: @user, task: task, title: "Focus: Deck",
                         start_at: 3.hours.from_now, end_at: 3.hours.from_now + 45.minutes, status: :dismissed)

      expect(task.held_block).to eq(earlier)
      expect(later.reload).to be_proposed
    end
  end
end

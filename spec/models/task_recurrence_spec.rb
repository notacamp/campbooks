require "rails_helper"

RSpec.describe Task, "recurrence" do
  before do
    Time.zone = "Europe/Lisbon"
    @ws   = Workspace.create!(name: "Recur WS")
    @user = @ws.users.create!(name: "Rex", email_address: "rex-recur@example.com", password: "password123")
  end

  def build_task(**attrs)
    @ws.tasks.create!({
      title:      "Water the plants",
      status:     :todo,
      priority:   :normal,
      created_by: @user,
      due_at:     Time.zone.local(2026, 7, 6, 9, 0), # Monday
      rrule:      "FREQ=WEEKLY"
    }.merge(attrs))
  end

  it "completing a recurring task spawns the next occurrence" do
    task = build_task
    expect { task.move_to_status!(:done, by: @user) }.to change { @ws.tasks.count }.by(1)

    nxt = @ws.tasks.where.not(id: task.id).sole
    expect(nxt.title).to eq("Water the plants")
    expect(nxt).to be_todo
    expect(nxt.rrule).to eq("FREQ=WEEKLY")
    expect(nxt.recurrence_parent_id).to eq(task.id)
    expect(nxt.due_at).to eq(Time.zone.local(2026, 7, 13, 9, 0))
    expect(task.reload).to be_done
  end

  it "the spawned occurrence carries priority, all_day, creator and tags" do
    tag  = Tag.create!(workspace: @ws, name: "home", color: "#22c55e")
    task = build_task(priority: :high, all_day: true)
    task.tags << tag

    task.move_to_status!(:done, by: @user)
    nxt = @ws.tasks.where.not(id: task.id).sole

    expect(nxt).to be_priority_high
    expect(nxt).to be_all_day
    expect(nxt.created_by).to eq(@user)
    expect(nxt.tags).to include(tag)
  end

  it "occurrences link flat to the series root" do
    root = build_task
    root.move_to_status!(:done, by: @user)
    second = @ws.tasks.where.not(id: root.id).sole

    second.move_to_status!(:done, by: @user)
    third = @ws.tasks.where.not(id: [ root.id, second.id ]).sole

    expect(second.recurrence_parent_id).to eq(root.id)
    expect(third.recurrence_parent_id).to eq(root.id), "grandchild links to the root, not its immediate parent"
    expect(third.due_at).to eq(Time.zone.local(2026, 7, 20, 9, 0))
  end

  it "completing an already-done task is a no-op (no double spawn)" do
    task = build_task
    task.move_to_status!(:done, by: @user)
    expect { task.move_to_status!(:done, by: @user) }.not_to change { @ws.tasks.count }
  end

  it "reopening then re-completing does not duplicate the next slot" do
    task = build_task
    task.move_to_status!(:done, by: @user)
    task.move_to_status!(:todo, by: @user)
    expect { task.move_to_status!(:done, by: @user) }.not_to change { @ws.tasks.count }
  end

  it "a one-off task does not spawn on completion" do
    task = build_task(rrule: nil)
    expect { task.move_to_status!(:done, by: @user) }.not_to change { @ws.tasks.count }
  end

  it "cancelling a recurring task does not spawn" do
    task = build_task
    expect { task.move_to_status!(:cancelled, by: @user) }.not_to change { @ws.tasks.count }
  end

  it "a bounded (COUNT) series stops producing occurrences" do
    task = build_task(rrule: "FREQ=WEEKLY;COUNT=1")
    expect { task.move_to_status!(:done, by: @user) }.not_to change { @ws.tasks.count }
  end

  it "a recurring task requires a due date" do
    task = @ws.tasks.build(title: "x", status: :todo, priority: :normal, rrule: "FREQ=WEEKLY", due_at: nil)
    expect(task).not_to be_valid
    expect(task.errors[:due_at]).to be_present
  end

  it "an unparseable rrule is rejected" do
    task = @ws.tasks.build(title: "x", status: :todo, priority: :normal, rrule: "not a rule", due_at: Time.current)
    expect(task).not_to be_valid
    expect(task.errors[:rrule]).to be_present
  end
end

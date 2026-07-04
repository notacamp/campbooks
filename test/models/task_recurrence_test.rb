require "test_helper"

class TaskRecurrenceTest < ActiveSupport::TestCase
  setup do
    Time.zone = "Europe/Lisbon"
    @ws = Workspace.create!(name: "Recur WS")
    @user = @ws.users.create!(name: "Rex", email_address: "rex-recur@example.com", password: "password123")
  end

  def build_task(**attrs)
    @ws.tasks.create!({
      title: "Water the plants",
      status: :todo,
      priority: :normal,
      created_by: @user,
      due_at: Time.zone.local(2026, 7, 6, 9, 0), # Monday
      rrule: "FREQ=WEEKLY"
    }.merge(attrs))
  end

  test "completing a recurring task spawns the next occurrence" do
    task = build_task
    assert_difference -> { @ws.tasks.count }, 1 do
      task.move_to_status!(:done, by: @user)
    end

    nxt = @ws.tasks.where.not(id: task.id).sole
    assert_equal "Water the plants", nxt.title
    assert nxt.todo?
    assert_equal "FREQ=WEEKLY", nxt.rrule
    assert_equal task.id, nxt.recurrence_parent_id
    assert_equal Time.zone.local(2026, 7, 13, 9, 0), nxt.due_at
    assert task.reload.done?
  end

  test "the spawned occurrence carries priority, all_day, creator and tags" do
    tag = Tag.create!(workspace: @ws, name: "home", color: "#22c55e")
    task = build_task(priority: :high, all_day: true)
    task.tags << tag

    task.move_to_status!(:done, by: @user)
    nxt = @ws.tasks.where.not(id: task.id).sole

    assert nxt.priority_high?
    assert nxt.all_day?
    assert_equal @user, nxt.created_by
    assert_includes nxt.tags, tag
  end

  test "occurrences link flat to the series root" do
    root = build_task
    root.move_to_status!(:done, by: @user)
    second = @ws.tasks.where.not(id: root.id).sole

    second.move_to_status!(:done, by: @user)
    third = @ws.tasks.where.not(id: [ root.id, second.id ]).sole

    assert_equal root.id, second.recurrence_parent_id
    assert_equal root.id, third.recurrence_parent_id, "grandchild links to the root, not its immediate parent"
    assert_equal Time.zone.local(2026, 7, 20, 9, 0), third.due_at
  end

  test "completing an already-done task is a no-op (no double spawn)" do
    task = build_task
    task.move_to_status!(:done, by: @user)
    assert_no_difference -> { @ws.tasks.count } do
      task.move_to_status!(:done, by: @user)
    end
  end

  test "reopening then re-completing does not duplicate the next slot" do
    task = build_task
    task.move_to_status!(:done, by: @user)
    task.move_to_status!(:todo, by: @user)
    assert_no_difference -> { @ws.tasks.count } do
      task.move_to_status!(:done, by: @user)
    end
  end

  test "a one-off task does not spawn on completion" do
    task = build_task(rrule: nil)
    assert_no_difference -> { @ws.tasks.count } do
      task.move_to_status!(:done, by: @user)
    end
  end

  test "cancelling a recurring task does not spawn" do
    task = build_task
    assert_no_difference -> { @ws.tasks.count } do
      task.move_to_status!(:cancelled, by: @user)
    end
  end

  test "a bounded (COUNT) series stops producing occurrences" do
    task = build_task(rrule: "FREQ=WEEKLY;COUNT=1")
    assert_no_difference -> { @ws.tasks.count } do
      task.move_to_status!(:done, by: @user)
    end
  end

  test "a recurring task requires a due date" do
    task = @ws.tasks.build(title: "x", status: :todo, priority: :normal, rrule: "FREQ=WEEKLY", due_at: nil)
    assert_not task.valid?
    assert task.errors[:due_at].present?
  end

  test "an unparseable rrule is rejected" do
    task = @ws.tasks.build(title: "x", status: :todo, priority: :normal, rrule: "not a rule", due_at: Time.current)
    assert_not task.valid?
    assert task.errors[:rrule].present?
  end
end

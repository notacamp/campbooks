require "test_helper"

class TaskTest < ActiveSupport::TestCase
  setup do
    @ws = Workspace.create!(name: "Tasks Test WS")
    @user = @ws.users.create!(name: "Tess", email_address: "tess-tasks@example.com", password: "password123")
  end

  test "requires a title" do
    task = @ws.tasks.build(status: :todo, priority: :normal)
    assert_not task.valid?
    assert_includes task.errors[:title], "can't be blank"
  end

  test "status defaults to suggested and priority to normal" do
    task = @ws.tasks.create!(title: "Triage me")
    assert_equal "suggested", task.status
    assert task.priority_normal?
  end

  test "fingerprint_for is deterministic and normalizes the title" do
    a = Task.fingerprint_for(source_type: "EmailMessage", source_id: "abc", title: "Pay Invoice")
    b = Task.fingerprint_for(source_type: "EmailMessage", source_id: "abc", title: "  pay invoice  ")
    assert_equal a, b
  end

  test "accessible_to is workspace-scoped and fails closed for a nil user" do
    mine = @ws.tasks.create!(title: "Mine")
    theirs = Workspace.create!(name: "Other WS").tasks.create!(title: "Theirs")

    assert_includes Task.accessible_to(@user), mine
    assert_not_includes Task.accessible_to(@user), theirs
    assert_empty Task.accessible_to(nil)
  end

  test "move_to_status! to done stamps completed_at and logs events" do
    task = @ws.tasks.create!(title: "Ship it", status: :todo, created_by: @user)

    task.move_to_status!(:done, by: @user)

    assert task.done?
    assert_not_nil task.completed_at
    names = Event.for_subject(task).pluck(:name)
    assert_includes names, "task.status_changed"
    assert_includes names, "task.completed"
  end

  test "move_to_status! away from done clears completed_at" do
    task = @ws.tasks.create!(title: "Reopen", status: :done, completed_at: Time.current, created_by: @user)

    task.move_to_status!(:in_progress, by: @user)

    assert task.in_progress?
    assert_nil task.completed_at
  end

  test "move_to_status! rejects an unknown status" do
    task = @ws.tasks.create!(title: "Nope", status: :todo)
    assert_raises(ArgumentError) { task.move_to_status!(:nonsense) }
  end

  test "overdue scope returns only active, past-due tasks" do
    overdue = @ws.tasks.create!(title: "late", status: :todo, due_at: 1.day.ago)
    upcoming = @ws.tasks.create!(title: "soon", status: :todo, due_at: 1.day.from_now)
    done_late = @ws.tasks.create!(title: "done late", status: :done, due_at: 1.day.ago)

    assert_includes Task.overdue, overdue
    assert_not_includes Task.overdue, upcoming
    assert_not_includes Task.overdue, done_late
  end

  test "shares the Tag model with emails through task_tags" do
    tag = Tag.create!(workspace: @ws, name: "finance", color: "#6366f1")
    task = @ws.tasks.create!(title: "Budget")
    task.tags << tag

    assert_includes task.reload.tags, tag
    assert_includes tag.reload.tasks, task
  end
end

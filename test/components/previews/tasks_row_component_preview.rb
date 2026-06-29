# frozen_string_literal: true

class TasksRowComponentPreview < ViewComponent::Preview
  # In-progress task with a future due date, a label and assignees.
  def default
    render Campbooks::Tasks::Row.new(task: sample_task)
  end

  # Overdue, urgent task — the due chip turns red and an URGENT flag shows.
  def overdue
    render Campbooks::Tasks::Row.new(task: sample_task(status: :todo, priority: :urgent, due_at: 2.days.ago))
  end

  # Completed task — title is struck through, no priority flag.
  def done
    render Campbooks::Tasks::Row.new(task: sample_task(status: :done, priority: :normal, due_at: nil))
  end

  private

  # An unsaved Task with its associations stubbed via singleton methods, so the
  # preview never touches the database (mirrors PipelineBoardCardComponentPreview).
  def sample_task(status: :in_progress, priority: :high, due_at: 2.days.from_now)
    task = Task.new(title: "Send Q3 report to the board", status: status, priority: priority, due_at: due_at)
    task.define_singleton_method(:tags) { [ Tag.new(name: "finance", color: "#6366f1") ] }
    task.define_singleton_method(:assignees) { [ User.new(name: "Alex Kim"), User.new(name: "Sam Lee") ] }
    task
  end
end

# frozen_string_literal: true

# Previews for the calendar TaskChip (mirrors ReminderChip): the compact grid chip
# and the agenda row, for a Scout-suggested and a manual task.
class TaskChipComponentPreview < ViewComponent::Preview
  # The compact pill used in month/week cells.
  def chip
    render Campbooks::Calendar::TaskChip.new(task: task)
  end

  # The agenda-row shape.
  def row
    render Campbooks::Calendar::TaskChip.new(task: task, variant: :row)
  end

  # A manually-created (non-suggested) task → the "Task" source label.
  def manual_task
    render Campbooks::Calendar::TaskChip.new(task: task(ai_suggested: false), variant: :row)
  end

  private

  def task(ai_suggested: true)
    Task.new(id: SecureRandom.uuid, title: "Review the Q3 budget", due_at: Time.current.change(hour: 16, min: 0),
             all_day: false, ai_suggested: ai_suggested, status: "todo")
  end
end

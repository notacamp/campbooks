# frozen_string_literal: true

class TasksStatusBadgeComponentPreview < ViewComponent::Preview
  # Every task status, in board order, so the color vocabulary is reviewable at a glance.
  def suggested
    render Campbooks::Tasks::StatusBadge.new(status: :suggested)
  end

  def todo
    render Campbooks::Tasks::StatusBadge.new(status: :todo)
  end

  def in_progress
    render Campbooks::Tasks::StatusBadge.new(status: :in_progress)
  end

  def blocked
    render Campbooks::Tasks::StatusBadge.new(status: :blocked)
  end

  def done
    render Campbooks::Tasks::StatusBadge.new(status: :done)
  end

  def cancelled
    render Campbooks::Tasks::StatusBadge.new(status: :cancelled)
  end
end

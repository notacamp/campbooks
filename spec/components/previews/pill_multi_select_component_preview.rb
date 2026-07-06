# frozen_string_literal: true

class PillMultiSelectComponentPreview < ViewComponent::Preview
  # Tags: a color dot per pill, one preselected.
  def tags
    render Campbooks::PillMultiSelect.new(name: "task[tag_ids][]", options: [
      { value: 1, label: "Invoices", color: "#f97316", checked: true },
      { value: 2, label: "Receipts", color: "#22c55e" },
      { value: 3, label: "Contracts", color: "#6366f1" },
      { value: 4, label: "Follow up", color: "#ec4899" }
    ])
  end

  # Assignees: an avatar per pill.
  def assignees
    render Campbooks::PillMultiSelect.new(name: "assignee_ids[]", options: [
      { value: 1, label: "Alice Smith", avatar: "Alice Smith", checked: true },
      { value: 2, label: "Bob Jones", avatar: "Bob Jones" },
      { value: 3, label: "Carla Nguyen", avatar: "Carla Nguyen", checked: true }
    ])
  end

  # Plain labels — no dot, no avatar.
  def plain
    render Campbooks::PillMultiSelect.new(name: "pick[]", options: [
      { value: "todo", label: "To do", checked: true },
      { value: "in_progress", label: "In progress" },
      { value: "blocked", label: "Blocked" },
      { value: "done", label: "Done" }
    ])
  end

  # None selected — the resting state.
  def empty
    render Campbooks::PillMultiSelect.new(name: "pick[]", options: [
      { value: 1, label: "First" },
      { value: 2, label: "Second" },
      { value: 3, label: "Third" }
    ])
  end
end

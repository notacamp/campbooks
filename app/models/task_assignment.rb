# Joins a Task to a workspace member it's assigned to (multiple per task). The
# creator is tracked separately on tasks.created_by_id; `assigned_by` records who
# made this particular assignment (for the task.assigned event / activity log).
class TaskAssignment < ApplicationRecord
  belongs_to :task
  belongs_to :user
  belongs_to :assigned_by, class_name: "User", optional: true

  validates :user_id, uniqueness: { scope: :task_id }
end

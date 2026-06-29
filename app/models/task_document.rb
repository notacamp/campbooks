# Attaches a workspace Document to a Task (mirrors task_email_links). One row per
# (task, document).
class TaskDocument < ApplicationRecord
  belongs_to :task
  belongs_to :document
  belongs_to :created_by, class_name: "User", optional: true

  validates :document_id, uniqueness: { scope: :task_id }
end

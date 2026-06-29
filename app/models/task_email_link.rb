# A typed relationship between a Task and an EmailMessage, beyond the task's
# polymorphic origin (Task#source). One relationship per (task, email).
class TaskEmailLink < ApplicationRecord
  belongs_to :task
  belongs_to :email_message
  belongs_to :created_by, class_name: "User", optional: true

  # Integer-backed — APPEND new values, never reorder existing.
  enum :relationship, { related: 0, reference: 1, follow_up: 2, blocked_by: 3 }

  validates :relationship, presence: true
  validates :email_message_id, uniqueness: { scope: :task_id }
end

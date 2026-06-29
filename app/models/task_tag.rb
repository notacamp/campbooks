# Joins a Task to a workspace Tag — the same labels emails use (mirrors
# email_message_tags). No change to the Tag model; the task label picker shows
# only local, non-system tags (Tag.where(email_account_id: nil).excluding_system_labels).
class TaskTag < ApplicationRecord
  belongs_to :task
  belongs_to :tag

  validates :tag_id, uniqueness: { scope: :task_id }
end

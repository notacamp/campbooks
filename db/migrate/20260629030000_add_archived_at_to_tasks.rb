# Soft-archive for tasks: hide from the active lists/board/feed without deleting.
# Orthogonal to status (an archived task keeps its done/blocked/etc. state).
class AddArchivedAtToTasks < ActiveRecord::Migration[8.1]
  def change
    add_column :tasks, :archived_at, :datetime
    add_index :tasks, [ :workspace_id, :archived_at ]
  end
end

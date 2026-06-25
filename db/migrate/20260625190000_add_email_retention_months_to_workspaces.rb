class AddEmailRetentionMonthsToWorkspaces < ActiveRecord::Migration[8.1]
  def change
    # Per-workspace, opt-in content retention: auto-delete email older than this many
    # months on the daily RetentionSweepJob. NULL = keep forever (the default, so
    # nothing changes for anyone who doesn't turn it on).
    add_column :workspaces, :email_retention_months, :integer
  end
end

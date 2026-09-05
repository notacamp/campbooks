# frozen_string_literal: true

# Asks can be put off for a week ("Not now") without changing their status — a
# snoozed ask disappears from Now/People/Time until the snooze lapses, then
# comes back through the normal live rules. One nullable column + a lookup index
# on (workspace, snoozed_until) so the awake/snoozed scopes stay indexed.
class AddSnoozedUntilToTasks < ActiveRecord::Migration[8.1]
  def change
    add_column :tasks, :snoozed_until, :datetime
    add_index :tasks, [ :workspace_id, :snoozed_until ]
  end
end

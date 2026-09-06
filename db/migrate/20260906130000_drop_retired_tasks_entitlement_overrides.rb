# frozen_string_literal: true

# Asks are core: the `tasks` billing entitlement left config/plans.yml, so a
# per-workspace override for it (granted by hand while tasks were a paid feature)
# is now an unknown key that Workspace#entitlement_overrides_valid would reject on
# the workspace's next save. Strip the retired key from every workspace here so an
# upgrade needs no manual step. Idempotent: jsonb `-` on a missing key is a no-op,
# and the WHERE keeps the write set to rows that actually carry it. Down is a
# deliberate no-op (the paid feature is gone; there is nothing to restore).
class DropRetiredTasksEntitlementOverrides < ActiveRecord::Migration[8.1]
  def up
    execute <<~SQL
      UPDATE workspaces
         SET entitlement_overrides = entitlement_overrides - 'tasks',
             updated_at = NOW()
       WHERE entitlement_overrides ? 'tasks'
    SQL
  end

  def down
    # No-op: the tasks entitlement no longer exists in the plan catalog.
  end
end

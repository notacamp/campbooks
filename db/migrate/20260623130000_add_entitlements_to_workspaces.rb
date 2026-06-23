class AddEntitlementsToWorkspaces < ActiveRecord::Migration[8.1]
  def change
    # Subscription tier (a key in config/plans.yml). Constant default backfills
    # existing rows on PG11+ as a metadata-only change.
    add_column :workspaces, :plan, :string, null: false, default: "free"

    # Per-workspace deviations from the plan defaults (grandfathering, manual
    # bumps, admin enable/disable). Validated against the composed JSON Schema.
    add_column :workspaces, :entitlement_overrides, :jsonb, null: false, default: {}

    add_index :workspaces, :plan
  end
end

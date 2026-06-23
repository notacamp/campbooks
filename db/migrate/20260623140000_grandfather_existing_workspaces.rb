class GrandfatherExistingWorkspaces < ActiveRecord::Migration[8.1]
  # Feature keys as of this rollout (snapshot — intentionally NOT read from the
  # evolving Entitlements catalog so replaying this migration stays stable).
  FEATURES = %w[
    email_accounts workflows managed_ai scout ai_model_access
    emails_synced workflow_executions notifications
  ].freeze

  # Grandfather workspaces that predate plan limits: keep them on "free" but unlock
  # every feature with no caps, via entitlement_overrides. Only touches workspaces
  # that have no overrides yet (every existing one at rollout); free workspaces
  # created afterwards keep the limited free defaults from config/plans.yml.
  def up
    grant_all = FEATURES.index_with { { "allowed" => true, "enabled" => true, "limit" => nil } }

    execute(<<~SQL.squish)
      UPDATE workspaces
      SET entitlement_overrides = #{connection.quote(grant_all.to_json)}::jsonb
      WHERE entitlement_overrides = '{}'::jsonb
    SQL
  end

  def down
    # Irreversible: a later-set override is indistinguishable from a grandfather one.
  end
end

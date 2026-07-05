# frozen_string_literal: true

# Until now the single `users.role` enum served two unrelated meanings: admin of
# YOUR workspace (invitation approval, restricted folders) and operator of the
# whole INSTANCE (/admin, /jobs — which are globally scoped). Splitting them:
# `role` stays the workspace-level role; the new `app_admin` flag marks instance
# operators. Both backfills are idempotent and self-healing — no manual steps
# for self-hosters.
class SeparateAppAdminsFromWorkspaceAdmins < ActiveRecord::Migration[8.1]
  def up
    add_column :users, :app_admin, :boolean, default: false, null: false

    # Existing role-admins were de-facto instance operators (only they could
    # reach /admin) — keep them operating after the split.
    execute "UPDATE users SET app_admin = TRUE WHERE role = 1"

    # Self-serve signup never granted the workspace creator the admin role, so
    # those workspaces have nobody to approve invitations or manage member
    # roles. Promote each adminless workspace's earliest user (its creator).
    execute <<~SQL
      UPDATE users SET role = 1 WHERE id IN (
        SELECT DISTINCT ON (u.workspace_id) u.id
        FROM users u
        WHERE u.workspace_id IS NOT NULL
          AND NOT EXISTS (
            SELECT 1 FROM users a
            WHERE a.workspace_id = u.workspace_id AND a.role = 1
          )
        ORDER BY u.workspace_id, u.created_at ASC
      )
    SQL
  end

  def down
    remove_column :users, :app_admin
  end
end

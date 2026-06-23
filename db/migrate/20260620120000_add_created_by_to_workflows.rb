class AddCreatedByToWorkflows < ActiveRecord::Migration[8.1]
  def change
    # The user who built the workflow. Email-action steps run "as" this user, so
    # EmailActions' per-user permission gates apply to automations too. Nullable:
    # workflows created before this column have no recorded owner, and their
    # email_action steps fail closed (access denied) until an owner is set.
    #
    # Idempotent: the column may already exist on databases where an earlier copy
    # of this migration ran under a since-renumbered timestamp.
    return if column_exists?(:workflows, :created_by_id)

    add_reference :workflows, :created_by, foreign_key: { to_table: :users }, null: true
  end
end

class CreateConnections < ActiveRecord::Migration[8.1]
  def change
    # A saved outbound HTTP integration (base URL + auth) referenced by workflow
    # `custom_action` steps. Auth is resolved server-side at run time so a secret
    # is never stored in a step's plaintext Liquid config.
    create_table :connections do |t|
      t.references :workspace, null: false, foreign_key: true
      t.string :name, null: false
      t.string :base_url, null: false
      t.string :auth_type, null: false, default: "none"
      t.string :auth_header_name             # for auth_type "header" (e.g. "X-Api-Key")
      t.string :auth_username                # for auth_type "basic"
      t.text :auth_secret                    # encrypted (token / password / header value)

      t.timestamps
    end

    add_index :connections, [ :workspace_id, :name ]
  end
end

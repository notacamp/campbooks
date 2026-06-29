class CreateEmailTemplates < ActiveRecord::Migration[8.1]
  def change
    create_table :email_templates, id: :uuid, default: -> { "gen_random_uuid()" } do |t|
      t.references :workspace, null: false, foreign_key: true, type: :uuid
      t.string :name, null: false
      t.text :description
      t.string :subject, null: false, default: ""
      t.text :body_html, null: false, default: ""
      t.jsonb :variables_schema, null: false, default: []
      t.integer :ai_status, null: false, default: 0
      t.jsonb :ai_provenance, null: false, default: {}
      t.timestamps
    end
    add_index :email_templates, %i[workspace_id name]
  end
end

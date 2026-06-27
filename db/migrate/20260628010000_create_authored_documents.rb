class CreateAuthoredDocuments < ActiveRecord::Migration[8.0]
  def change
    create_table :authored_documents, id: :uuid do |t|
      t.references :workspace, null: false, foreign_key: true, type: :uuid
      t.string :title, null: false
      t.text :html_content
      t.references :author, foreign_key: { to_table: :users }, type: :uuid
      t.timestamps
    end

    add_index :authored_documents, %i[workspace_id created_at]
  end
end

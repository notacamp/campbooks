class CreateAuthoredDocuments < ActiveRecord::Migration[8.1]
  def change
    create_table :authored_documents do |t|
      t.references :workspace, null: false, foreign_key: true
      t.string :title, null: false
      t.text :html_content
      t.references :author, foreign_key: { to_table: :users }
      t.timestamps
    end

    add_index :authored_documents, %i[workspace_id created_at]
  end
end

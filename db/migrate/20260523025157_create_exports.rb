class CreateExports < ActiveRecord::Migration[8.1]
  def change
    create_table :exports do |t|
      t.references :organization, null: false, foreign_key: true
      t.integer :status, default: 0, null: false
      t.integer :documents_count, default: 0
      t.jsonb :filters, default: {}

      t.timestamps
    end
  end
end

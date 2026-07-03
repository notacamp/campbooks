class CreateAiPrompts < ActiveRecord::Migration[8.1]
  def change
    create_table :ai_prompts, id: :uuid do |t|
      t.references :workspace, null: false, foreign_key: true, type: :uuid
      t.string :purpose, null: false
      t.text :instructions

      t.timestamps
    end

    add_index :ai_prompts, %i[workspace_id purpose], unique: true
  end
end

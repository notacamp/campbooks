class CreateAiConfigurations < ActiveRecord::Migration[8.1]
  def change
    create_table :ai_configurations do |t|
      t.string :purpose, null: false
      t.string :provider, null: false
      t.string :model, null: false
      t.string :api_key
      t.string :endpoint_url
      t.integer :max_tokens, null: false, default: 1000
      t.float :temperature, null: false, default: 0.0
      t.boolean :enabled, null: false, default: true
      t.jsonb :extra_settings, null: false, default: {}

      t.timestamps
    end
    add_index :ai_configurations, :purpose, unique: true
  end
end

class MoveModelAndTokensToAiConfigurations < ActiveRecord::Migration[8.1]
  def up
    add_column :ai_configurations, :model, :string, null: false, default: ""
    add_column :ai_configurations, :max_tokens, :integer, null: false, default: 1000
    add_column :ai_configurations, :temperature, :float, null: false, default: 0.0

    # Backfill from adapters
    execute <<-SQL
      UPDATE ai_configurations
      SET model = ai_adapters.model,
          max_tokens = ai_adapters.max_tokens,
          temperature = ai_adapters.temperature
      FROM ai_adapters
      WHERE ai_configurations.ai_adapter_id = ai_adapters.id
    SQL

    remove_column :ai_adapters, :model
    remove_column :ai_adapters, :max_tokens
    remove_column :ai_adapters, :temperature
  end

  def down
    add_column :ai_adapters, :model, :string, null: false, default: ""
    add_column :ai_adapters, :max_tokens, :integer, null: false, default: 1000
    add_column :ai_adapters, :temperature, :float, null: false, default: 0.0

    # Backfill from first config per adapter
    execute <<-SQL
      UPDATE ai_adapters
      SET model = COALESCE(ac.model, ''),
          max_tokens = COALESCE(ac.max_tokens, 1000),
          temperature = COALESCE(ac.temperature, 0.0)
      FROM (
        SELECT DISTINCT ON (ai_adapter_id) ai_adapter_id, model, max_tokens, temperature
        FROM ai_configurations
        ORDER BY ai_adapter_id, id
      ) ac
      WHERE ai_adapters.id = ac.ai_adapter_id
    SQL

    remove_column :ai_configurations, :model
    remove_column :ai_configurations, :max_tokens
    remove_column :ai_configurations, :temperature
  end
end

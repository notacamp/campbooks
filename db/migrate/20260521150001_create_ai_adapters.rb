class CreateAiAdapters < ActiveRecord::Migration[8.0]
  def up
    create_table :ai_adapters do |t|
      t.references :organization, null: false, foreign_key: true
      t.string :name, null: false
      t.string :provider, null: false
      t.string :model, null: false
      t.string :api_key
      t.string :endpoint_url
      t.integer :max_tokens, null: false, default: 1000
      t.float :temperature, null: false, default: 0.0
      t.jsonb :extra_settings, null: false, default: {}
      t.boolean :enabled, null: false, default: true
      t.timestamps
    end

    add_index :ai_adapters, [ :organization_id, :name ], unique: true

    # Add ai_adapter_id column before migrating data
    add_reference :ai_configurations, :ai_adapter, foreign_key: { to_table: :ai_adapters }

    # Migrate existing ai_configurations into ai_adapters, deduplicating by
    # (organization_id, provider, model, api_key, endpoint_url, max_tokens, temperature)
    say_with_time "migrating ai_configurations into ai_adapters" do
      conn = ActiveRecord::Base.connection.raw_connection

      rows = conn.exec(<<~SQL)
        SELECT DISTINCT ON (organization_id, provider, model, COALESCE(api_key, ''), COALESCE(endpoint_url, ''), max_tokens, temperature)
          organization_id, provider, model, api_key, endpoint_url, max_tokens, temperature
        FROM ai_configurations
        ORDER BY organization_id, provider, model, COALESCE(api_key, ''), COALESCE(endpoint_url, ''), max_tokens, temperature, id
      SQL

      rows.each do |row|
        name = "#{row['provider'].humanize} — #{row['model']}"
        result = conn.exec_params(
          "SELECT COUNT(*) FROM ai_adapters WHERE organization_id = $1 AND name = $2",
          [ row["organization_id"].to_i, name ]
        )
        count = result.first["count"].to_i
        name = "#{name} (#{count + 1})" if count > 0

        result = conn.exec_params(<<~SQL,
          INSERT INTO ai_adapters (organization_id, name, provider, model, api_key, endpoint_url, max_tokens, temperature, enabled, created_at, updated_at)
          VALUES ($1, $2, $3, $4, $5, $6, $7, $8, true, NOW(), NOW())
          RETURNING id
        SQL
          [ row["organization_id"].to_i, name, row["provider"], row["model"],
            row["api_key"], row["endpoint_url"],
            row["max_tokens"].to_i, row["temperature"].to_f ]
        )
        adapter_id = result.first["id"].to_i

        conn.exec_params(<<~SQL,
          UPDATE ai_configurations
          SET ai_adapter_id = $1
          WHERE organization_id = $2
            AND provider = $3
            AND model = $4
            AND COALESCE(api_key, '') = COALESCE($5, '')
            AND COALESCE(endpoint_url, '') = COALESCE($6, '')
            AND max_tokens = $7
            AND temperature = $8
            AND ai_adapter_id IS NULL
        SQL
          [ adapter_id, row["organization_id"].to_i, row["provider"], row["model"],
            row["api_key"], row["endpoint_url"], row["max_tokens"].to_i, row["temperature"].to_f ]
        )
      end
    end

    # Remove old provider-specific columns from ai_configurations
    change_table :ai_configurations do |t|
      t.remove :provider
      t.remove :model
      t.remove :api_key
      t.remove :endpoint_url
      t.remove :max_tokens
      t.remove :temperature
      t.remove :extra_settings
    end
  end

  def down
    change_table :ai_configurations do |t|
      t.string :provider
      t.string :model
      t.string :api_key
      t.string :endpoint_url
      t.integer :max_tokens, null: false, default: 1000
      t.float :temperature, null: false, default: 0.0
      t.jsonb :extra_settings, null: false, default: {}
    end

    say_with_time "restoring ai_configurations from ai_adapters" do
      conn = ActiveRecord::Base.connection.raw_connection
      conn.exec(<<~SQL)
        UPDATE ai_configurations
        SET provider = ai_adapters.provider,
            model = ai_adapters.model,
            api_key = ai_adapters.api_key,
            endpoint_url = ai_adapters.endpoint_url,
            max_tokens = ai_adapters.max_tokens,
            temperature = ai_adapters.temperature,
            extra_settings = ai_adapters.extra_settings
        FROM ai_adapters
        WHERE ai_configurations.ai_adapter_id = ai_adapters.id
      SQL

      conn.exec(<<~SQL)
        UPDATE ai_configurations
        SET provider = 'deepseek',
            model = 'deepseek-chat',
            max_tokens = 1000,
            temperature = 0.0
        WHERE provider IS NULL
      SQL
    end

    remove_column :ai_configurations, :ai_adapter_id
    drop_table :ai_adapters
  end
end

class AddManagedToAiAdapters < ActiveRecord::Migration[8.1]
  # Marks an adapter whose API key is the Campbooks platform's (resolved from the
  # provider env key at call time), not the workspace's own. Lets cloud users pick
  # "Campbooks AI" instead of bringing their own key. Existing adapters all carry a
  # user key, so the false default is correct — no data migration needed.
  def change
    add_column :ai_adapters, :managed, :boolean, null: false, default: false
  end
end

class ChangeAiConfigurationsMaxTokensDefault < ActiveRecord::Migration[8.1]
  def change
    # 1000 is too low for reasoning models (e.g. deepseek-v4-pro), which can
    # exhaust the budget on reasoning before emitting any content. Lift the
    # default and bump existing rows that were left at the old default.
    change_column_default :ai_configurations, :max_tokens, from: 1000, to: 4000

    up_only do
      execute "UPDATE ai_configurations SET max_tokens = 4000 WHERE max_tokens = 1000"
    end
  end
end

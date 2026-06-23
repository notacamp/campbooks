class BackfillAiConfigurationSystemPrompts < ActiveRecord::Migration[8.1]
  def up
    AiConfiguration.where(system_prompt: nil).find_each do |config|
      rich_text = ActionText::RichText.find_by(
        record_type: "AiConfiguration",
        record_id: config.id,
        name: "system_prompt"
      )
      config.update_column(:system_prompt, rich_text.body.to_html) if rich_text&.body.present?
    end
  end

  def down
    # Irreversible but data remains in action_text_rich_texts
  end
end

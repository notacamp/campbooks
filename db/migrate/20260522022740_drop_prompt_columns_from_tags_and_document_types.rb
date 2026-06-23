class DropPromptColumnsFromTagsAndDocumentTypes < ActiveRecord::Migration[8.1]
  def change
    remove_column :tags, :prompt, :text
    remove_column :document_types, :prompt, :text
    remove_column :ai_configurations, :system_prompt, :text
  end
end

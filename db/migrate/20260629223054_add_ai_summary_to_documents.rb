class AddAiSummaryToDocuments < ActiveRecord::Migration[8.1]
  def change
    add_column :documents, :ai_summary, :text
  end
end

class AddAiAnalysisCommentRefToEmailMessages < ActiveRecord::Migration[8.1]
  def change
    add_reference :email_messages, :ai_analysis_comment, foreign_key: { to_table: :email_comments }
  end
end

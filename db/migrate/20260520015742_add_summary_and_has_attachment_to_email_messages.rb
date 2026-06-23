class AddSummaryAndHasAttachmentToEmailMessages < ActiveRecord::Migration[8.1]
  def change
    add_column :email_messages, :summary, :text
    add_column :email_messages, :has_attachment, :boolean
  end
end

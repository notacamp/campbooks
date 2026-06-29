class AddEmailTemplateToScheduledEmails < ActiveRecord::Migration[8.1]
  def change
    add_reference :scheduled_emails, :email_template, null: true, foreign_key: true
  end
end

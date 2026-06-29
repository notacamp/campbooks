class AddEmailTemplateToScheduledEmails < ActiveRecord::Migration[8.1]
  def change
    add_reference :scheduled_emails, :email_template, null: true, foreign_key: true, type: :uuid
    add_column :scheduled_emails, :template_context, :jsonb, null: false, default: {}
  end
end

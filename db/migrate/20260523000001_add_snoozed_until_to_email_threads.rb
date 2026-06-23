class AddSnoozedUntilToEmailThreads < ActiveRecord::Migration[8.1]
  def change
    add_column :email_threads, :snoozed_until, :datetime
    add_index :email_threads, :snoozed_until,
              where: "snoozed_until IS NOT NULL",
              name: "index_email_threads_on_snoozed_until_not_null"
  end
end

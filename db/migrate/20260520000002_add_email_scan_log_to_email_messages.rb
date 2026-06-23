class AddEmailScanLogToEmailMessages < ActiveRecord::Migration[8.1]
  def change
    add_reference :email_messages, :email_scan_log, null: false, foreign_key: true
  end
end

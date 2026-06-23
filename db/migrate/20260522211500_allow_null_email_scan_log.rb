class AllowNullEmailScanLog < ActiveRecord::Migration[8.1]
  def change
    change_column_null :email_messages, :email_scan_log_id, true
  end
end

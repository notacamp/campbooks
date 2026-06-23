class DropMonthlyReports < ActiveRecord::Migration[8.1]
  def change
    drop_table :monthly_reports
  end
end
